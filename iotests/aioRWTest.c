#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <stdlib.h>
#include <getopt.h>
#include <syslog.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>
#include <assert.h>
#include <math.h>

#include "aioRequests.h"
#include "utils.h"
#include "logSpeed.h"
#include "diskStats.h"
#include "positions.h"
#include "cigar.h"
#include "devices.h"
#include "blockVerify.h"

int    keepRunning = 1;       // have we been interrupted
double exitAfterSeconds = 60;
int    qd = 256;
int    qdSpecified = 0;
char   *dataLog = NULL;
char   *benchLog = NULL;
int    dataLogFormat = 0;
int    seqFiles = 1;
int    seqFilesSpecified = 0;
size_t maxSizeInBytes = 0;
size_t alignment = 0;
size_t LOWBLKSIZE = 65536;
size_t BLKSIZE = 65536;
int    jumpStep = 0;
int    rrSpecified = 0;
double readRatio = 0.5;
size_t table = 0;
char   *logFNPrefix = NULL;
int    verbose = 0;
int    singlePosition = 0;
size_t    flushEvery = 0;
int    verifyWrites = 0;
char*  specifiedDevices = NULL;
int    sendTrim = 0;
long   startAtZero = -99999; // -99999 is a special case for random start
size_t maxPositions = 0;
size_t dontUseExclusive = 0;
int    blocksFromEnd = 0;
char*  logPositions = NULL;
long int seed;
char*  cigarPattern = NULL;
cigartype cigar;
int    oneShot = 0;
char   *randomBufferFile = NULL;
int    fsyncAfterWriting = 0;
char   *description = NULL;
int    dontExitOnErrors = 0;
int    sizeOverride = 0;
size_t contextCount = 1;
size_t waitEvery = 0;
size_t cyclick = 0;

deviceDetails *deviceList = NULL;
size_t deviceCount = 0;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}



void handle_args(int argc, char *argv[]) {
  int opt;
  seed = (long int) (timedouble());
  if (seed < 0) seed=-seed;
  seed = seed & 0xffff; // only one of 65536 values
  srand48(seed);
  
  while ((opt = getopt(argc, argv, "t:k:K:o:q:f:s:G:p:Tl:vVS:FR:O:rwb:MgzP:Xa:L:I:D:JB:C:1Z:Nd:Ec:W:")) != -1) {
    switch (opt) {
    case 'a':
      alignment = atoi(optarg) * 1024;
      alignment = (alignment >> 1) << 1;
      break;
    case 'd':
      description = strdup(optarg);
      fprintf(stderr,"*info* description: '%s'\n", description);
      break;
    case 'N':
      fsyncAfterWriting = 1;
      break;
    case 'J':
      dataLogFormat = JSON;
      break;
    case 'D':
      dataLog = strdup(optarg);
      break;
    case 'E':
      dontExitOnErrors = 1;
      break;
    case 'B':
      benchLog = strdup(optarg);
      break;
    case 'L':
      logPositions = strdup(optarg);
      break;
    case 'I':
      {}
      size_t added = loadDeviceDetails(optarg, &deviceList, &deviceCount);
      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
      //      inputFilenames = strdup(optarg);
      break;
    case 'X':
      dontUseExclusive++;
      break;
    case 'P':
      maxPositions = atoi(optarg);
      if (maxPositions > 0) 
	fprintf(stderr,"*info* hard coded maximum number of positions %zd\n", maxPositions);
      break;
    case 'z':
      startAtZero = 0;
      break;
    case 'Z':
      startAtZero = atol(optarg);
      if (startAtZero < 0 && startAtZero != -99999) startAtZero = labs(startAtZero);
      break;
    case 'M':
      dataLogFormat = MYSQL;
      break;
    case 'T':
      table = 1;
      break;
    case 'O':
      if (!specifiedDevices) {
	specifiedDevices = strdup(optarg);
      }
      break;
      //    case '0':
      //      maxPositions = 0;
      //      break;
    case 'r':
      readRatio += 0.5;
      if (readRatio > 1) readRatio = 1;
      rrSpecified = 1;
      break;
    case 'w':
      readRatio -= 0.5;
      if (readRatio < 0) readRatio = 0;
      rrSpecified = 1;
      break;
    case 'R':
      seed = atol(optarg);
      srand48(seed);
      break;
    case 'F':
      if (flushEvery == 0) {
	flushEvery = 1;
      } else {
	flushEvery = 10 * flushEvery;
      }
      if (flushEvery > 1L<<30) {
	flushEvery = 1L<<30;
      }
      break;
    case 'W':
      waitEvery = atoi(optarg);
      break;
    case 'v':
      verifyWrites = 1;
      break;
    case 'V':
      verbose++;
      break;
    case 'l':
      logFNPrefix = strdup(optarg);
      break;
    case 't':
      exitAfterSeconds = atof(optarg); 
      break;
    case 'q':
      qd = atoi(optarg); if (qd < 1) qd = 1;
      qdSpecified = 1;
      break;
    case 's':
      seqFiles = atoi(optarg);
      seqFilesSpecified = 1;
      break;
    case 'S':
      randomBufferFile = strdup(optarg);
      fprintf(stderr,"*info* randomBufferFromFile '%s'\n", randomBufferFile);
      break;
    case 'b':
      blocksFromEnd = atoi(optarg);
      fprintf(stderr,"*info* blockoffset: %d\n", blocksFromEnd);
      break;
    case 'j':
      jumpStep = atoi(optarg); 
      break;
    case 'G':
      sizeOverride = 1;
      if ((strlen(optarg)>=1) && (optarg[strlen(optarg)-1] == 'R')) {
	maxSizeInBytes = totalRAM() * atof(optarg);
	if (maxSizeInBytes < 1) maxSizeInBytes = 1*1024*1024*1024;
	fprintf(stderr,"*info* setting -G to be %.1lf GiB\n", TOGiB(maxSizeInBytes));
      } else {
	maxSizeInBytes = atof(optarg) * 1024*1024*1024;
      }
      break;
    case 'k': {
      char *ndx = index(optarg, '-');
      if (ndx) {
	int firstnum = 1 << (int)log2(atof(optarg) * 1024);
	int secondnum = 1 << (int)log2(atof(ndx+1) * 1024);
	if (secondnum < firstnum) secondnum = firstnum;
	if (verbose > 1) {
	  fprintf(stderr,"*info* specific block range: %g KiB (%d) to %g KiB (%d)\n", firstnum/1024.0, firstnum, secondnum/1024.0, secondnum);
	}
	LOWBLKSIZE = firstnum;
	BLKSIZE = secondnum;
	// range
      } else {
	BLKSIZE = 1 << (int)log2(atof(optarg) * 1024);
	LOWBLKSIZE = BLKSIZE;
      }}
      break;
    case 'K':
      cyclick = atoi(optarg);
      break;
    case 'p':
      readRatio = atof(optarg);
      if (readRatio < 0 || readRatio > 1) {fprintf(stderr,"*error* -p should be in the range [0..1]. Maybe you meant -P\n"); exit(-1);}
      rrSpecified = 1;
      break;
    case 'f':
      addDeviceDetails(optarg,  &deviceList, &deviceCount);
      break;
    case 'C':
      cigarPattern = strdup(optarg);
      cigar_init(&cigar);
      cigar_setrwrand(&cigar, readRatio);
      if (cigar_parse(&cigar, optarg) != 0) {
	fprintf(stderr,"*error* not a valid CIGAR pattern\n");
	exit(-1);
	//      } else {
	//	fprintf(stderr,"*info* CIGAR: ");
	//	cigar_dump(&cigar, stderr);
	//        fprintf(stderr,"\n");
      }
      break;
    case 'c':
      //      contextCount = atoi(optarg);
      if (contextCount < 1) contextCount = 1;
      break;
    case '1':
      fprintf(stderr,"*info* write to locations only once\n");
      oneShot = 1;
      break;
    default:
      exit(-1);
    }
  }

  if (deviceCount < 1) {
    fprintf(stderr,"./aioRWTest -f device\n");
    fprintf(stderr,"\nExample:\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0            # 50/50 read/write test, seq r/w\n");
    fprintf(stderr,"  ./aioRWTest -q 128 -f /dev/device   # set the queue depth to 128\n");
    fprintf(stderr,"  ./aioRWTest -c 8 -f /dev/device     # change the number of IO contexts to 8\n");
    fprintf(stderr,"  ./aioRWTest -I devicelist.txt       # 50/50 read/write test, from a file\n");
    fprintf(stderr,"  ./aioRWTest -r -f /dev/nbd0         # read test\n");
    fprintf(stderr,"  ./aioRWTest -w -f /dev/nbd0         # write test\n");
    fprintf(stderr,"  ./aioRWTest -w -FF -f /dev/nbd0     # write test, flush every 10 writes.\n");
    fprintf(stderr,"  ./aioRWTest -w -v -f /dev/nbd0      # write test with verify\n");
    fprintf(stderr,"  ./aioRWTest -w -V -f /dev/nbd0      # same including showing the 1st N pos\n");
    fprintf(stderr,"  ./aioRWTest -r -s1 -I devs.txt      # read test, single contiguous region\n");
    fprintf(stderr,"  ./aioRWTest -w -s128 -f /dev/nbd0   # 128 parallel contiguous regions\n");
    fprintf(stderr,"  ./aioRWTest -P1 -F -f /dev/nbd0     # single static position, fsync after\n");
    fprintf(stderr,"  ./aioRWTest -p0.25 -f /dev/nbd0     # 25%% read, and 75%% write\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -G100      # limit actions to first 100GiB\n");
    fprintf(stderr,"  ./aioRWTest -p0.1 -f/dev/nbd0 -G3   # 10%% r/w ratio, limit to 3 GiB\n");
    fprintf(stderr,"  ./aioRWTest -t30 -f/dev/nbd0        # test for 30 seconds\n");
    fprintf(stderr,"  ./aioRWTest -0 -F -f /dev/nbd0      # send no operations, then flush. \n");
    fprintf(stderr,"  ./aioRWTest -P1 -F -V -f /dev/nbd0  # verbose that shows every operation\n");
    fprintf(stderr,"  ./aioRWTest -P1 -F -V -f file.txt   # can also use a single file.\n");
    //    fprintf(stderr,"  ./aioRWTest -C CIGAR -f /dev/nbd0   # Use CIGAR format for R/W/X actions. '100R' '200X' '10R100W50X'\n");
    //    fprintf(stderr,"  ./aioRWTest -C CIGAR -f /dev/nbd0   # more examples, variable sizes '~R' '@W' ':W' '~R@W:W'\n");
    //    fprintf(stderr,"  ./aioRWTest -C CIGAR -f /dev/nbd0   # more examples, 'B' repeats the previous, 'S' skip\n");
    fprintf(stderr,"  ./aioRWTest -S file -f /dev/nbd0    # specify the block from 'S'\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -N         # call fsync() after writing\n");
    fprintf(stderr,"  ./aioRWTest -f filename -G10        # create a 10GiB file if not present\n");
    fprintf(stderr,"  ./aioRWTest -f filename -G 4R       # append R and it'll be 4x the RAM\n");
    fprintf(stderr,"  ./aioRWTest -v -t15 -p0.5 -f /dev/nbd0  # random pos, 50%% R/W, verified\n");
    fprintf(stderr,"  ./aioRWTest -v -t15 -p0.5 -R 9812 -f /dev/nbd0 # set the seed to 9812\n");
    //    fprintf(stderr,"  ./aioRWTest -s 1 -j 10 -f /dev/sdc -V          #  contiguous access, jumping 10 blocks at a time\n");
    fprintf(stderr,"  ./aioRWTest -s -8 -f /dev/sdc -V    # reverse contiguous 8 regions in parallel\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -O ok      # list of devs in ok.txt for disk stats/\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -k1 -G0.1      # write 1KiB into 100 MiB\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -P 10000 -f /dev/nbd0 -k1   # Use 10,000 positions.\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -XXX           # Triple X skips O_EXCL!\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -XXX -z        # 1st pos at 0\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -P10000 -z -a1 # align operations to 1KiB\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -Z 100         # start at block 100\n");
    fprintf(stderr,"  ./aioRWTest -D timedata -s1 -w -f /dev/nbd0    # log *block* timing and total\n");
    fprintf(stderr,"  ./aioRWTest -J -D timedata -s1 -w -f /dev/nbd0 # JSON format block timing\n");
    fprintf(stderr,"  ./aioRWTest -B benchmark -s1 -w -f /dev/nbd0   # log *per second* timing\n");
    fprintf(stderr,"  ./aioRWTest -L locations -s1 -w -f /dev/nbd0   # dump ops to 'locations'\n");
    fprintf(stderr,"  ./aioRWTest -G 10 -w -t-1 -1                   # -1 write 10GiB once then stop\n");
    fprintf(stderr,"  ./aioRWTest -W 2                               # -W wait for 2 seconds between ops\n");
    fprintf(stderr,"  ./aioRWTest -k 4 -K 512                        # 4 kiB random buffer, 512 bytes cycle\n");
    fprintf(stderr,"  ./verify < locations                           # verify write operation \n");
    fprintf(stderr,"\nTable summary:\n");
    fprintf(stderr,"  ./aioRWTest -T -t 2 -f /dev/nbd0  # table of various parameters\n");
    exit(-1);
  }
}


int main(int argc, char *argv[]) {
#ifndef VERSION
#define VERSION __TIMESTAMP__
#endif

  fprintf(stderr,"*exit* use spit -f device -c commandstring\n");exit(1);
  
  fprintf(stderr,"*info* stutools %s %s \n", argv[0], VERSION);
    
  handle_args(argc, argv);
  if (exitAfterSeconds < 0) {
    exitAfterSeconds = 99999999;
  }

  if (maxSizeInBytes == 0) {maxSizeInBytes = totalRAM() * 2;}
  if (verbose)
    fprintf(stderr,"*info* sizeOverride %d, maxSizeInBytes: %zd\n", sizeOverride, maxSizeInBytes);

  size_t swap = swapTotal();
  if (swap) {fprintf(stderr,"*warning* swap is enabled (%.1lf GiB). This isn't ideal for benchmarking.\n", TOGiB(swap));}

  char *cli = (char *)malloc(1);
  cli[0] = 0;
  
  for(size_t i = 0; i < argc; i++) {
    cli = (char *)realloc(cli, strlen(cli) + strlen(argv[i]) + 2); // space 0 
    strcat(cli, argv[i]);
    strcat(cli, " ");
  }

  if (!table) { // if not table mode then install ^c handlers
    signal(SIGTERM, intHandler);
    signal(SIGINT, intHandler);
  }


  diskStatType dst; // count sectors/bytes
  diskStatSetup(&dst);
  if (specifiedDevices) {
    diskStatFromFilelist(&dst, specifiedDevices, verbose);
  }

  // fix up alignment
  if (alignment <= 0) {
    alignment = LOWBLKSIZE;
  }
  const int alignbits = (int)(log(alignment)/log(2) + 0.01);
  if (1<<alignbits != alignment) {
    fprintf(stderr,"*error* alignment of %zd not suitable, changing to %d\n", alignment, 1<<alignbits);
    alignment = 1<< alignbits;
  }
  if (alignment > LOWBLKSIZE) {
    alignment = LOWBLKSIZE;
  }

  //  expandDevices(&deviceList, &deviceCount, &seqFiles, &maxSizeInBytes); // if files and -s then expand

  
  size_t sz = 0;
  if (seqFiles == 0) {
    sz = alignedNumber((size_t)(maxSizeInBytes), 1<<16);
  } else {
    sz = alignedNumber((size_t)(maxSizeInBytes/seqFiles), 1<<16);
  }

  size_t origdc = deviceCount;
  for (size_t i = 0; i < origdc; i++) {
    if (sizeOverride) {
      deviceList[i].shouldBeSize = sz;
    } else {
      // get from size
      deviceList[i].shouldBeSize = maxSizeInBytes;
      if (fileExists(deviceList[i].devicename)) {
	int isBD = isBlockDevice(deviceList[i].devicename);
	if (isBD == 1) {
	  deviceList[i].shouldBeSize = blockDeviceSize(deviceList[i].devicename);
	} else if (isBD == 2) {
	  deviceList[i].shouldBeSize = fileSizeFromName(deviceList[i].devicename);
	}
	//	fprintf(stderr,"%d...%zd %zd %zd\n", isBD, i, deviceList[i].shouldBeSize, maxSizeInBytes);
      }
    }
    if (isBlockDevice(deviceList[i].devicename) != 1) {
      char str[1000];
      for (size_t j = 2; j <= seqFiles; j++) {
	sprintf(str, "%s_%zd", deviceList[i].devicename, j);
	deviceDetails *d = addDeviceDetails(str, &deviceList, &deviceCount);
	//d->bdSize = sz;
	d->shouldBeSize = sz;
	if (verbose >= 2) fprintf(stderr,"*info* expanding %s, %zd\n", d->devicename, d->shouldBeSize);
      }
    }
  }

  if (contextCount > deviceCount) contextCount = deviceCount;
  if (verbose) {
    fprintf(stderr,"*info* contextCount = %zd, deviceCount = %zd\n", contextCount, deviceCount);
  }
  io_context_t *ioc = createContexts(contextCount, qd);
  setupContexts(ioc, contextCount, qd);
  openDevices(deviceList, deviceCount, sendTrim, &maxSizeInBytes, LOWBLKSIZE, BLKSIZE, alignment, readRatio < 1, dontUseExclusive, qd, contextCount);
  // if we have specified bigger than the BD then don't set it too high
  for (size_t i = 0; i <deviceCount; i++) {
    if (deviceList[i].shouldBeSize > deviceList[i].bdSize) {
      deviceList[i].shouldBeSize = deviceList[i].bdSize;
    }
  }

  // prune closed, char or too small
  deviceDetails *dd2 = prune(deviceList, &deviceCount, BLKSIZE);
  freeDeviceDetails(deviceList, deviceCount);
  deviceList = dd2;

  size_t bdSizeWeAreUsing = smallestBDSize(deviceList, deviceCount);
  if (maxSizeInBytes > 0 && (bdSizeWeAreUsing > maxSizeInBytes)) {
    bdSizeWeAreUsing = maxSizeInBytes;
  }
  infoDevices(deviceList, deviceCount);
  
  if (verbose >= 1) {
    fprintf(stderr,"*info* using bdSize of %zd (%.3lf GiB)\n", bdSizeWeAreUsing, TOGiB(bdSizeWeAreUsing));
  }
  
  int anyopen = 0;
  for (size_t j = 0; j < deviceCount; j++) {
    if (deviceList[j].fd > 0) {
      anyopen = 1;
    }
  }
  if (!anyopen) {
    fprintf(stderr,"*error* there are no valid block devices\n");
    exit(-1);
  }

  if (verbose >= 1) {
    infoDevices(deviceList, deviceCount);
  }

  if (maxPositions == 0) {
    // not set yet, figure it out
    maxPositions = maxSizeInBytes / BLKSIZE;
    fprintf(stderr,"*info* maximum position count set to %zd (%.3f GiB / %zd bytes)\n", maxPositions, TOGiB(maxSizeInBytes), BLKSIZE);
    if (maxPositions > 1000000*exitAfterSeconds) {
      maxPositions = 1000000*exitAfterSeconds;
      fprintf(stderr,"*info* positions limited to %zd assuming max 1M IOPS for %.0lf seconds\n", maxPositions, exitAfterSeconds);
    }
    if (maxPositions*sizeof(positionType) > totalRAM() / 8) {
      maxPositions = totalRAM() / 8 / sizeof(positionType);
      fprintf(stderr,"*info* positions limited to %zd due to RAM constraints\n", maxPositions);
    }
  } else {
    fprintf(stderr,"*info* maximum position count set to %zd\n", maxPositions);
  }

  if ((maxPositions % deviceCount) != 0) {
    size_t newmp = (maxPositions / deviceCount) + 1;
    newmp *= deviceCount;
    if (verbose >= 2)
      fprintf(stderr,"*info* changing %zd to be %zd\n", maxPositions, newmp);
    maxPositions = newmp;
  }

  if (alignment == 0) {
    alignment = LOWBLKSIZE;
  }

  // using the -G option to reduce the max position on the block device

  fprintf(stderr,"*info* seed = %ld", seed);
  if (cigar_len(&cigar)) {
    fprintf(stderr, ", cigar = %s ('", cigarPattern);
    cigar_dump(&cigar, stderr);
    fprintf(stderr, "')");
  }
  fprintf(stderr,"\n");

  char *randomBuffer;
  CALLOC(randomBuffer, BLKSIZE, 1);
  memset(randomBuffer, 0, BLKSIZE);
  
  if (!randomBufferFile) {
    generateRandomBufferCyclic(randomBuffer, BLKSIZE, seed, cyclick);
  } else {
    int f = open(randomBufferFile, O_RDONLY);
    if (f < 0) {perror(randomBufferFile);exit(-1);}
    int fr = read(f, randomBuffer, BLKSIZE);
    if (fr < BLKSIZE) {
      fprintf(stderr,"*warning* file size of '%s' is less than %zd. Filling with 0x00.\n", randomBufferFile, BLKSIZE);
    }
  }
  
  positionType *positions = createPositions(maxPositions);

  int exitcode = 0;
  
  size_t row = 0;
  if (table) {
    // generate a table
    size_t bsArray[]={BLKSIZE};
    double *rrArray = NULL;
    if(rrSpecified){
      rrSpecified = 1;
      CALLOC(rrArray, rrSpecified, sizeof(double));
      rrArray[0] = readRatio;
    } else {
      rrSpecified = 3;
      CALLOC(rrArray, rrSpecified, sizeof(double));
      rrArray[0] = 1.0; rrArray[1] = 0; rrArray[2] = 0.5;
    }

    size_t *qdArray = NULL;
    if (qdSpecified) {
      qdSpecified = 1;
      CALLOC(qdArray, qdSpecified, sizeof(size_t));
      qdArray[0] = qd;
    } else {
      qdSpecified = 3;
      CALLOC(qdArray, qdSpecified, sizeof(size_t));
      qdArray[0] = 1; qdArray[1] = 32; qdArray[2] = 256;
    }
      
    size_t *ssArray = NULL; 
    if (seqFilesSpecified) { // if 's' specified on command line, then use it only 
      seqFilesSpecified = 1;
      CALLOC(ssArray, seqFilesSpecified, sizeof(size_t));
      ssArray[0] = seqFiles;
    } else { // otherwise an array of values
      seqFilesSpecified = 4;
      CALLOC(ssArray, seqFilesSpecified, sizeof(size_t));
      ssArray[0] = 0; ssArray[1] = 1; ssArray[2] = 32; ssArray[3] = 128;
    }

    fprintf(stderr," blkSz   numSq  QueueD     R/W    IOPS  MiB/s  Ampli   Disk%%\n");
    
    for (size_t rrindex=0; rrindex < rrSpecified; rrindex++) {
      for (size_t ssindex=0; ssindex < seqFilesSpecified; ssindex++) {
	for (size_t qdindex=0; qdindex < qdSpecified; qdindex++) {
	  for (size_t bsindex=0; bsindex < sizeof(bsArray) / sizeof(bsArray[0]); bsindex++) {
	    size_t rb = 0, ios = 0, totalWB = 0, totalRB = 0;
	    double start = 0, elapsed = 0;
	    char filename[1024];

	    if (logFNPrefix) {
	      mkdir(logFNPrefix, 0755);
	    }
	    sprintf(filename, "%s/bs%zd_ss%zd_qd%zd_rr%.2f", logFNPrefix ? logFNPrefix : ".", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    logSpeedType l;
	    logSpeedInit(&l);

	    diskStatStart(&dst); // reset the counts
	    
	    fprintf(stderr,"%6zd  %6zd  %6zd  %6g ", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    
	    if (ssArray[ssindex] == 0) {
	      // setup random positions. An value of 0 means random. e.g. zero sequential files
	      setupPositions(positions, &maxPositions, deviceList, deviceCount, 0, rrArray[rrindex], bsArray[bsindex], bsArray[bsindex], alignment, singlePosition, jumpStep, startAtZero, bdSizeWeAreUsing, blocksFromEnd, &cigar, seed);

	      start = timedouble(); // start timing after positions created
	      rb = aioMultiplePositions(positions, maxPositions, exitAfterSeconds, qdArray[qdindex], 0, 1, &l, NULL, randomBuffer, bsArray[bsindex], alignment, &ios, &totalRB, &totalWB, oneShot, dontExitOnErrors, ioc, contextCount);
	    } else {
	      // setup multiple/parallel sequential region
	      setupPositions(positions, &maxPositions, deviceList, deviceCount, ssArray[ssindex], rrArray[rrindex], bsArray[bsindex], bsArray[bsindex], alignment, singlePosition, jumpStep, startAtZero, bdSizeWeAreUsing, blocksFromEnd, &cigar, seed);

	      
	      start = timedouble(); // start timing after positions created
	      rb = aioMultiplePositions(positions, maxPositions, exitAfterSeconds, qdArray[qdindex], 0, 1, &l, NULL, randomBuffer, bsArray[bsindex], alignment, &ios, &totalRB, &totalWB, oneShot, dontExitOnErrors, ioc, contextCount);
	    }
	    if (verbose) {
	      fprintf(stderr,"*info* calling fsync()..."); fflush(stderr);
	    }
	    for (size_t f = 0; f < deviceCount; f++) {
	      if (deviceList[f].fd > 0) {
		fsync(deviceList[f].fd); // should be parallel sync
	      }
	    }
	    if (verbose) {
	      fprintf(stderr,"\n");
	    }

	    elapsed = timedouble() - start;
	      
	    diskStatFinish(&dst);

	    size_t trb = 0, twb = 0;
	    double util = 0;
	    diskStatSummary(&dst, &trb, &twb, &util, 0, 0, 0, elapsed);	    
	    size_t shouldHaveBytes = rb;
	    size_t didBytes = trb + twb;
	    double efficiency = didBytes *100.0/shouldHaveBytes;

	    logSpeedDump(&l, filename, dataLogFormat, description, bdSizeWeAreUsing, bdSizeWeAreUsing, rrArray[rrindex], flushEvery, ssArray[ssindex], bsArray[bsindex], bsArray[bsindex], cli);
	    logSpeedFree(&l);
	    
	    //	    fprintf(stderr,"%6.0lf\t%6.0lf\t%6.0lf\t%6.0lf\n", ios/elapsed, TOMiB(ios*BLKSIZE/elapsed), efficiency, util);
	    fprintf(stderr," %6.0lf %6.0lf ", ios/elapsed, TOMiB(ios * BLKSIZE/elapsed));
	    if (specifiedDevices) {
	      fprintf(stderr,"%6.0lf", efficiency);
	    } else {
	      fprintf(stderr,"   n/a");
	    }
	    fprintf(stderr, " %7.0lf\n", util);
	    row++;
	    if (row > 1) {
	      //	      rrindex=99999;ssindex=99999;qdindex=99999;bsindex=99999;
	    }
	  }
	}
      }
    }
    free(ssArray); ssArray = NULL;

    // end table results
  } else {
    // just execute a single run
    size_t totl = diskStatTotalDeviceSize(&dst);

      
    fprintf(stderr,"*info* seq: %d, r/w Ratio: %.1g, qd: %d, block size: %zd-%zd bytes\n*info* aligned to %zd bytes\n", seqFiles, readRatio, qd, LOWBLKSIZE, BLKSIZE, alignment);
    fprintf(stderr,"*info* flush %zd, max bdSizeWeAreUsing %zd (%.2lf GiB), off %d, contexts %zd\n", flushEvery, bdSizeWeAreUsing, TOGiB(bdSizeWeAreUsing), blocksFromEnd, contextCount);
    if (totl > 0) {
      fprintf(stderr,"*info* origBDSize %.3lf GiB, sum rawDiskSize %.3lf GiB (overhead %.1lf%%)\n", TOGiB(bdSizeWeAreUsing), TOGiB(totl), 100.0*totl/bdSizeWeAreUsing - 100);
    }
    //    assert(maxPositions > 0);
    setupPositions(positions, &maxPositions, deviceList, deviceCount, seqFiles, readRatio, LOWBLKSIZE, BLKSIZE, alignment, singlePosition, jumpStep, startAtZero, bdSizeWeAreUsing, blocksFromEnd, &cigar, seed);

    if (verbose >= 1) {
      positionStats(positions, maxPositions, deviceList, deviceCount);
    }

    /* if (logPositions) { */
    /*   fprintf(stderr, "*info* writing predefined positions to '%s'\n", logPositions); */
    /*   savePositions(logPositions, pathArray, pathLen, positions, maxPositions, bdSizeWeAreUsing, flushEvery, seed); */
    /* } */


    logSpeedType benchl;
    logSpeedInit(&benchl);

    diskStatStart(&dst); // grab the sector counts

    logSpeedType l;
    double start = logSpeedInit(&l);

    size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
    aioMultiplePositions(positions, maxPositions, exitAfterSeconds, qd, verbose, 0, dataLog ? (&l) : NULL, &benchl, randomBuffer, BLKSIZE, alignment, &ios, &shouldReadBytes, &shouldWriteBytes, oneShot, dontExitOnErrors, ioc, contextCount);

    if (fsyncAfterWriting && shouldWriteBytes) { // only fsync if we are told to AND we have written something
      fprintf(stderr,"*info* calling fsync()...");fflush(stderr);
      for (size_t f = 0; f < deviceCount; f++) {
	if (deviceList[f].fd > 0) 
	  fsync(deviceList[f].fd); // should be parallel sync
      }
      fprintf(stderr,"\n");
    }
    
    double elapsed = timedouble() - start;

    if (logPositions) { 
      fprintf(stderr, "*info* writing positions to '%s'\n", logPositions); 
      savePositions(logPositions, positions, maxPositions, flushEvery); 
    } 
    

    if (dataLog) {
      fprintf(stderr, "*info* writing per-block timing to '%s'\n", dataLog);
      logSpeedDump(&l, dataLog, dataLogFormat, description, bdSizeWeAreUsing, bdSizeWeAreUsing, readRatio, flushEvery, seqFiles, LOWBLKSIZE, BLKSIZE, cli);
    }

    if (benchLog) {
      fprintf(stderr, "*info* writing per-second benchmark speeds to '%s'\n", benchLog);
      logSpeedDump(&benchl, benchLog, dataLogFormat, description, bdSizeWeAreUsing, bdSizeWeAreUsing, readRatio, flushEvery, seqFiles, LOWBLKSIZE, BLKSIZE, cli);
    }

    logSpeedFree(&l);
    logSpeedFree(&benchl);

    
    diskStatFinish(&dst); // and sector counts when finished

    char s[1000];
    sprintf(s, "r: %.1lf GiB, %.0lf MiB/s, w: %.1lf GiB, %.0lf MiB/s, %.1lf s, qd %d, bs %zd-%zd, seq %d, drives %zd (%.1lf GiB)",
	    TOGiB(shouldReadBytes), TOMiB(shouldReadBytes)/elapsed,
	    TOGiB(shouldWriteBytes), TOMiB(shouldWriteBytes)/elapsed, elapsed, qd, LOWBLKSIZE, BLKSIZE, seqFiles, deviceCount, TOGiB(bdSizeWeAreUsing));

    fprintf(stderr,"*info* r: %.1lf GiB, %.0lf MiB/s, w: %.1lf GiB, %.0lf MiB/s, %.1lf s\n", TOGiB(shouldReadBytes), TOMiB(shouldReadBytes)/elapsed,
	    TOGiB(shouldWriteBytes), TOMiB(shouldWriteBytes)/elapsed, elapsed);
    
    char *user = username();
    syslog(LOG_INFO, "%s - %s stutools %s", s, user, VERSION);
    free(user);


    /* number of bytes read/written not under our control */
    size_t trb = 0, twb = 0;
    double util = 0;
    diskStatSummary(&dst, &trb, &twb, &util, shouldReadBytes, shouldWriteBytes, 1, elapsed);

    // if we want to verify, we iterate through the successfully completed IO events, and verify the writes
    if (verifyWrites) {
      exitcode = 0;
      if (readRatio < 1) {
	keepRunning = 1;
	//	int numerrors = aioVerifyWrites(positions, maxPositions, BLKSIZE, alignment, verbose, randomBuffer);
	size_t correct = 0, incorrect = 0, ioerrors = 0, lenerrors = 0;
	int numerrors = verifyPositions(positions, maxPositions, randomBuffer, 256, seed, BLKSIZE, &correct, &incorrect, &ioerrors, &lenerrors);

	fprintf(stderr,"*info* verify: total %zd, ok %zd, wrong %zd, ioerrors %zd, lenerrors %zd\n", correct+incorrect+ioerrors+lenerrors, correct, incorrect, ioerrors, lenerrors);
	
	if (numerrors) {
	  exitcode = MIN(numerrors, 254);
	  if (exitcode) {fprintf(stderr,"*warning* exit code %d\n", exitcode);}
	}
      }
    } else {
      // not verify so set exit code. The result is 100 = 10 GB/s, 10 = 1 GB/s, 1 = 0.100 GB/s
      //      exitcode = (int) (((TOMiB(shouldReadBytes) + TOMiB(shouldWriteBytes)) / elapsed) / 100.0 + 0.5);
      exitcode = 0; // if not verify, always exit with error code of 0
    }

    for (size_t f = 0; f < deviceCount; f++) {
      if (deviceList[f].fd > 0)
	close(deviceList[f].fd); // should be parallel sync
    }

  } // end single run

  diskStatFree(&dst);
  free(positions);
  free(randomBuffer);
  freeContexts(ioc, contextCount);
  freeDeviceDetails(deviceList, deviceCount);
  //  if (fdArray) free(fdArray);
  //  if (pathLen) {
  //    for(size_t i = 0; i < pathLen;i++)
  //      free(pathArray[i]);
  //  }
  //  if (pathArray) free(pathArray);
  if (logFNPrefix) free(logFNPrefix);
  if (dataLog) free(dataLog);
  if (benchLog) free(benchLog);
  if (specifiedDevices) free(specifiedDevices);
  if (logPositions) free(logPositions);
  if (cigarPattern) free(cigarPattern);
  if (description) free(description);
  if (cli) free(cli);
  
  return exitcode;
}
