#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>
#include <assert.h>
#include <string.h>
#include <unistd.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <sys/ipc.h>
#include <sys/shm.h>
#include <sys/stat.h>        /* For mode constants */
#include <fcntl.h>           /* For O_* constants */
#include <semaphore.h>

#include "keyvalue.h"
#include "utilstime.h"
#include "utils.h"
#include "simpsock.h"

int keepRunning = 1;

char *shared_name(char *namespace, int port) {
  if (strlen(namespace) >32) {
    namespace[100] = 32;
  }
  char s[255];
  sprintf(s, "/edrive-%s-%d", namespace, port);
  return strdup(s);
}

char *shared_mem(key_t shmid) {
  char *shm = NULL;
  if ((shm = shmat(shmid, NULL, 0)) == (char *) -1) {
    perror("shared_mem");
    return NULL;
  }
  return shm;
}

char *shared_getmem(char *bdname, key_t shmid, size_t *size, int *fd) {
  char *shm = NULL;
  if (bdname == NULL) {
    if (shmid != (key_t)-1) {
      // get the ram disk
      fprintf(stderr, "shmid: %d\n", shmid);
      shm = shared_mem(shmid);
      if (!shm) {
	printf("no shared memory exists\n");
	exit(EXIT_FAILURE);
      }
      
      struct shmid_ds buf;
      if(shmctl(shmid, IPC_STAT, &buf) < 0) {
	fprintf(stderr, "Could not close memory segment.\n");
      }
      *size = buf.shm_segsz;
     }
  } else {
    *fd = open(bdname, O_RDWR | O_EXCL | O_DIRECT) ;
    if (*fd < 0) {
      perror(bdname);
      exit(EXIT_FAILURE);
    }
    *size = blockDeviceSizeFromFD(*fd);
    fprintf(stderr,"name: %s, size: %zd\n", bdname, *size);
    shm = (char*)mmap(NULL, *size, PROT_READ | PROT_WRITE, MAP_SHARED_VALIDATE, *fd, 0);
    assert(shm);
  }
  return shm;
}

    

void shared_init(char *shm, char *name, int port, long size) {

  // clear first 16MB
  const long dataoffset = 16*1024*1024;
  memset(shm, 0, dataoffset);
  
  keyvalueType *kv = keyvalueInit();
  keyvalueAddUUID(kv);
  keyvalueSetLong(kv, "port", port);
  keyvalueAddString(kv, "deviceName", name);
  keyvalueAddString(kv, "deviceType", "RAM");
  keyvalueSetLong(kv, "sizeB", size);
  keyvalueSetLong(kv, "sizeGB", size*1.0/1024/1024/1024);
  keyvalueSetLong(kv, "dataoffset", dataoffset);
  keyvalueSetLong(kv, "createdTime", timeAsDouble());
  char *cpu = getCPUModel();
  keyvalueSetString(kv, "hostCPU", cpu);
  char *osr = OSRelease();
  keyvalueSetString(kv, "hostOSrelease", osr);
  free(osr);
  size_t ram = totalRAM();
  keyvalueSetLong(kv, "hostRAMGB", ram/1024.0/1024/1024);
  char *host = hostname();
  keyvalueSetString(kv, "hostname", host);
  free(host);
  size_t uptime = getUptime() / 24 / 3600;
  keyvalueSetLong(kv, "hostUptimeDays", uptime);
  
  keyvalueDumpAtStartRAM(kv, shm);
  //      sem_t *sem = (sem_t*)(shm + dataoffset - sizeof(sem_t));
  //      sem_init(sem, 1, 1);
}


void usage(void) {
  printf("usage: edrive <command> -n space/RAMGB -p <port> -e <nic> -s <serverip> -l <lat_ms>\n");
  printf("\n");
  printf("  edrive init -b <device> -n test/5 -p 1600 -e eth0 -s 127.0.0.1\n");
  printf("  edrive init -B <RAM shmid> -n test/5 -p 1600 -e eth0 -s 127.0.0.1\n");
  printf("  edrive list -n test/5 -p 1600\n");
  printf("  edrive rm -n test/5 -p 1600\n");
  printf("  edrive work -n test/5 -p 1600 -s 127.0.0.1\n");
  printf("  edrive dd -n test/5 -p 1600\n");
  printf("  edrive sb -n test/5 -p 1600\n");
  printf("  edrive size -n test/5 -p 1600   # data size, excluding superblock\n");
  printf("  edrive fullsize -n test/5 -p 1600   # data size, include superblock\n");
  printf("\n");
  printf("ipc/shared mem:\n");
  printf("  ipcs -m                     # lists all active ipc segments/shmid\n");
  printf("  ipcrm -m                    # delete a ipc segment/shmid\n");
  printf("  ipcmk -M $[1024*1024*1024]  # make a ipc segment/shmid\n");
  printf("\n");
  printf("sb:\n");
  printf("  edrive dd... | head\n");
}


int main(int argc, char *argv[]) {

  int opt = 0;
  int port = 1600;
  char *namespace = NULL, *nic = NULL, *server = NULL, *bdname = NULL;
  key_t bd_shmid = 0;
  size_t delayus = 4000;
  

  while ((opt = getopt(argc, argv, "n:p:e:s:b:B:l:")) != -1) {
    switch (opt) {
    case 'b':
      bdname = strdup(optarg);
      break;
    case 'B':
      bd_shmid = atoi(optarg);
      break;
    case 'l':
      delayus = atof(optarg) * 1000;
      break;
    case 'n':
      namespace = strdup(optarg);
      break;
    case 'p':
      port = atoi(optarg);
      break;
    case 'e':
      nic = strdup(optarg);
      break;
    case 's':
      server = strdup(optarg);
      break;
    default:
      usage();
      exit(EXIT_FAILURE);
    }
  }

  if (!namespace) {
    usage();
    exit(EXIT_FAILURE);
  }

  if (optind >= argc) {
    fprintf(stderr, "missing command\n");
    exit(EXIT_FAILURE);
  }

    
  char *command = argv[optind];


  char *name = strtok(namespace, "/");
  char *second = NULL;
  size_t ramgb = 0;

  if (name) second = strtok(NULL, "/");
  if (second) {
    ramgb = atoi(second);
    if (ramgb < 1) ramgb = 1;
  } else {
    if (strcmp(command, "register") == 0) {
      printf("need ram size\n");
      usage();
      exit(EXIT_FAILURE);
    }
  }

  fprintf(stderr, "namespace: %s, RAM: %zd GB, port %d, latency %.1lf ms (1:10,000 10x), nic: %s\n", name, ramgb, port, delayus/1000.0, nic);

  // do the commands


  size_t dataoffset = 16*1024*1024;
  
  if (strcmp(command, "init")==0) {
    size_t sz = 0;
    int fd;
    char *shm = shared_getmem(bdname, bd_shmid, &sz, &fd);
    fprintf(stderr,"initializing %zd bytes\n", sz);
    if (bdname) {
      shared_init(shm, bdname, port, sz);
    } else {
      char *newname = shared_name(name, port);
      shared_init(shm, newname, port, sz);
      free(newname);
    }
    munmap(shm, sz);
    close(fd);
  } else if (strcmp(command, "sb") == 0) {
    size_t sz = 0;
    int fd;
    char *shm = shared_getmem(bdname, bd_shmid, &sz, &fd);
    
    char *sb = strdup(shm);
    keyvalueType *kv = keyvalueInitFromString(sb);
    size_t cs = kv->checksum;
    size_t newcs = keyvalueChecksum(kv);
    
    if (cs != newcs) {
      fprintf(stderr,"not a valid sb %zd %zd\n", cs, newcs);
    } else {
      char *dump = keyvalueDumpAsJSON(kv);
      printf("%s", dump);
      free(dump);
    }
    keyvalueFree(kv);
    munmap(shm, sz);
    close(fd);
  } else if (strcmp(command, "size") == 0) {
  } else if (strcmp(command, "fullsize") == 0) {
  } else if (strcmp(command, "dd") == 0) {
    int fd;
    size_t sz = 0;
    char *shm = shared_getmem(bdname, bd_shmid, &sz, &fd);
    
    // don'dump the superblock
    for (size_t o = 0 /*dataoffset*/; o < sz; o+= 4096) {
      write(1, shm + o, 4096);
    }
    munmap(shm, sz);
    close(fd);
  } else if (strcmp(command, "work") == 0) {
    size_t sz = 0;
    int fd;
    char *shm = shared_getmem(bdname, bd_shmid, &sz, &fd);

    // check the superblock
    char *sb = strdup((char*)shm);
    keyvalueType *kv = keyvalueInitFromString(sb);
    size_t cs = kv->checksum;
    size_t newcs = keyvalueChecksum(kv);
    
    if (cs != newcs) {
      fprintf(stderr,"not a valid sb %zd %zd\n", cs, newcs);
      exit(EXIT_FAILURE);
    } 
    
    // grab the data offset
    dataoffset = keyvalueGetLong(kv, "dataoffset"); // start at the data place
    printf("dataoffset: %zd\n", dataoffset);
    assert(dataoffset >= 16*1024*1024);
    
    keyvalueSetLong(kv, "lastOpened", timeAsDouble());
    keyvalueDumpAtStartRAM(kv, shm);


    if (bdname) {
      munmap(shm, sz);
      close(fd);
      shm = NULL;
    }

    
    //volatile char *data = shm + dataoffset;
    // go for it!
    // start working
    
    //      sem_t *sem = (sem_t*)(data - sizeof(sem_t));
    //	sem_wait(sem);

    
    ///  sem_post(sem);

    int diskfd = 0;
    
    if(bdname) {
      diskfd = open(bdname, O_RDWR | O_EXCL | O_DIRECT) ;
      if (diskfd < 0) {
	perror(bdname);
	exit(EXIT_FAILURE);
      }
      fprintf(stderr,"name: %s, size: %zd\n", bdname, sz);
    }

    
    char header[1000];
    sprintf(header, "GET / HTTP/1.1\nHost: %s:%d\nUser-Agent: edrive-%s/0.0\nAccept */*\n", server, port, keyvalueGetString(kv, "deviceName"));
    
    char *buffer = aligned_alloc(4096, 1024*1024);
    memset(buffer, 0, 1024*1024);
    
    size_t writeHead = dataoffset;
    size_t maxBytes = keyvalueGetLong(kv, "sizeB");
    assert(maxBytes > 0);

    size_t iterations = 0;

    double lastUpdateSB = 0;
    
    const unsigned long randTime = 100 + (getDevRandomLong() % 100);
    
    size_t writtenBytes = keyvalueGetLong(kv, "writtenBytes");
    const size_t loadedWrittenBytes = writtenBytes;
    size_t lastWrittenBytes = writtenBytes;

    double starttime = timeAsDouble();
    while (1) {
      const double thistime = timeAsDouble();
      iterations++;
      int sockfd = sockconnect(server, port, 0);
      if (sockfd == -1) {
	perror("sockconnect");
	sleep(1);
	continue;
	//	  exit(1);
      }
      sprintf(header, "GET / HTTP/1.1\nHost: %s:%d\nX-WriteAtPosition: %zd\nUser-Agent: edrive-%s/0.0\nAccept */*\n", server, port, writeHead, keyvalueGetString(kv, "deviceName"));

      if ((iterations % 100) == 0) {
	fprintf(stderr,"pos: %zd / %zd / %.2lf (SB to go %.1lf)%%\n", writeHead - 16*1024*1024, maxBytes, ((writtenBytes - loadedWrittenBytes) / 1024.0 / 1024.0) / (thistime - starttime), randTime - (thistime - lastUpdateSB));
      }

      if (socksend(sockfd, header, 0, 1) < 0)
	perror("socksend");
      
      int rz =0, first = 0;

      while ((rz = sockrec(sockfd, buffer, 1024*1024-1, 0, 1)) > 0) {

		
	
	if (writeHead + rz <= maxBytes) {

	  if (bdname) {
	    if (pwrite(diskfd, buffer, 4096L*(rz/4096 + 1), 4096L*(writeHead/4096 + 1)) < 0) {
	      perror("pwrite");
	    }
	  } else {
	    // if not a real device, add fake
	    if (first++ == 0) {
	      if (delayus) {
		// 1 in 1,000 sleep 10s
		if ((getDevRandomLong() % 10000) == 0) {
		  fprintf(stderr,"fake IO retries...%.1lf ms\n", delayus * 20 /1000.0);
		}
		// always
		usleep(delayus); // 4ms fake delay
	      }

	    }
	    //shm mem
	    memcpy(shm + writeHead, buffer, rz);
	    //	    msync(shm +writeHead, rz, MS_SYNC);
	  }
	  writeHead += rz;

	  writtenBytes += rz;

	} else {
	  fprintf(stderr,"nah, off the end, resetting\n");
	  writeHead = dataoffset;
	}
	//	  printf("size: %d\n", rz);
      }

      


      if (thistime - lastUpdateSB > randTime) { // every 10 mins
	keyvalueSetLong(kv, "writtenByteTime", thistime);
	char flst[20];
	sprintf(flst, "%.2lf", writtenBytes * 1.0 / maxBytes);
	
	keyvalueSetString(kv, "writtenByteTDRatio", flst);
	keyvalueSetLong(kv, "writtenBytes", writtenBytes);
	keyvalueSetLong(kv, "writtenByteSpeedMBps", ((writtenBytes - lastWrittenBytes * 1.0) / randTime) / 1024.0 / 1024);
	if (bdname) {
	  keyvalueDumpAtStartFD(kv, diskfd);
	} else {
	  keyvalueDumpAtStartRAM(kv, shm);
	}
	//	fprintf(stderr,"WB: %zd\n", writtenBytes);
	lastWrittenBytes = writtenBytes;
	lastUpdateSB = thistime;
      }
      
      
      
      if (rz < 0) {
	perror("sockrec");
      }
      sockclose(sockfd);
    } // while 1
    
  } // work
  
  free(name);
  free(nic);



  return 0;
}
