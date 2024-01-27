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

int keepRunning = 1;

char *shared_name(char *namespace, int port) {
  if (strlen(namespace) >32) {
    namespace[100] = 32;
  }
  char s[255];
  sprintf(s, "/edrive-%s-%d", namespace, port);
  return strdup(s);
}


// return shmid
key_t shared_create(char *name, int port, long size) {
  char *s = shared_name(name, port);

  key_t shmid = -1;
  int fd = shm_open(s, O_RDWR | O_CREAT| O_EXCL, 0600);

  long dataoffset = 16*1024*1024;
  
  if (fd < 0) {
    perror(s);
    free(s); 
    return -1;
  } else {
    // successfully created the
    fprintf(stderr, "created: %s\n", s);
    // successfully created the
    // now create a shmem
    if ((shmid = shmget(IPC_PRIVATE, size + dataoffset, IPC_CREAT | IPC_EXCL | 0600)) < 0) {
      // failed, delete shm too
      if (shm_unlink(s) < 0) {
	perror(s);
      }
      free(s); s= NULL;
      
      perror("creating shm");
      exit(1);
    } else {
      free(s); s= NULL;

      // write the shmid to the shared file
      write(fd, &shmid, sizeof(shmid));
      close(fd);


      char *shm = NULL;
      if ((shm = shmat(shmid, (const void *)0x80000, 0)) == (char *) -1) {
        perror("shmat");
        return 1;
      }
      memset(shm, 0, size);

      keyvalueType *kv = keyvalueInit();
      keyvalueAddUUID(kv);
      keyvalueSetLong(kv, "port", port);
      keyvalueAddString(kv, "namespace", name);
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
      //      sem_t *sem = (sem_t*)shm;
      //      sem_init(sem, 1, 1);
    }
  }
  return shmid;
}

key_t shared_open(char *name, int port) {
  char *s = shared_name(name, port);
  
  key_t k = -1;
  int fd = shm_open(s, O_RDWR, 0600);
  if (fd < 0) {
    perror(s);
  } else {
    read(fd, &k, sizeof(k));
    close(fd);
  }
  free(s); s=NULL;
  
  return k;
}

char *shared_mem(key_t shmid) {
  char *shm = NULL;
  if ((shm = shmat(shmid, NULL, 0)) == (char *) -1) {
    perror("shared_mem");
  }
  return shm;
}
  

void shared_delete(key_t shmid, char *name, int port) {
  char *s = shared_name(name, port);

  if (shm_unlink(s) < 0) {
    perror("unlock");
  }

  if(shmctl(shmid, IPC_RMID, NULL) < 0) {
    fprintf(stderr, "Could not close memory segment.\n");
  }

  free(s); s=NULL;

}


void usage() {
  printf("usage: edrive <command> -n space/RAMGB -p <port> -e <nic> -s <serverip>\n");
  printf("\n");
  printf("  edrive register -n test/5 -p 1600 -e eth0 -s 127.0.0.1\n");
  printf("  edrive list -n test/5 -p 1600\n");
  printf("  edrive rm -n test/5 -p 1600\n");
  printf("  edrive work -n test/5 -p 1600 -s 127.0.0.1\n");
  printf("  edrive dd -n test/5 -p 1600\n");
  printf("  edrive sb -n test/5 -p 1600\n");
  printf("  edrive size -n test/5 -p 1600   # data size, excluding superblock\n");
  printf("  edrive fullsize -n test/5 -p 1600   # data size, include superblock\n");
  printf("\n");
  printf("ipc/shared mem:\n");
  printf("  ipcs -m   # lists all active ipc segments/shmid\n");
  printf("  ipcrm -m  # delete a ipc segment/shmid\n");
  printf("\n");
  printf("sb:\n");
  printf("  edrive dd... | head\n");
}


int main(int argc, char *argv[]) {

  int opt = 0;
  int port = 1600;
  char *namespace = NULL, *nic = NULL;

  while ((opt = getopt(argc, argv, "n:p:e:")) != -1) {
    switch (opt) {
    case 'n':
      namespace = strdup(optarg);
      break;
    case 'p':
      port = atoi(optarg);
      break;
    case 'e':
      nic = strdup(optarg);
      break;
    default:
      usage();
      exit(EXIT_FAILURE);
    }
  }

  if (!namespace || !nic) {
    usage();
    exit(EXIT_FAILURE);
  }

  if (optind >= argc) {
    fprintf(stderr, "missing command\n");
    exit(EXIT_FAILURE);
  }

    


  char *name = strtok(namespace, "/");
  char *second = NULL;
  size_t ramgb = 0;

  if (name) second = strtok(NULL, "/");
  if (second) {
    ramgb = atoi(second);
    if (ramgb < 1) ramgb = 1;
  } else {
    usage();
    exit(EXIT_FAILURE);
  }

  fprintf(stderr, "namespace: %s, RAM: %zd GB, port %d, nic: %s\n", name, ramgb, port, nic);

  // do the commands

  char *command = argv[optind];

  size_t dataoffset = 16*1024*1024;
  
  if (strcmp(command, "register")==0) {
    key_t shmid = shared_create(name, port, 1024L*1024*1024*ramgb);
    fprintf(stderr, "shmid: %d\n", shmid);
  } else if (strcmp(command, "list") == 0) {
    key_t shmid = shared_open(name, port);
    fprintf(stderr, "shmid: %d\n", shmid);
  } else if (strcmp(command, "rm") == 0) {
    key_t shmid = shared_open(name, port);
    fprintf(stderr, "shmid: %d\n", shmid);
    shared_delete(shmid, name, port);
  } else if (strcmp(command, "sb") == 0) {
    key_t shmid = shared_open(name, port);
    if (shmid != (key_t)-1) {
      fprintf(stderr, "shmid: %d\n", shmid);
      char *shm = shared_mem(shmid);
      if (!shm) {
	fprintf(stderr, "no shared memory exists\n");
	exit(EXIT_FAILURE);
      }
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
    } else {
      exit(EXIT_FAILURE);
    }
  } else if (strcmp(command, "size") == 0) {
    key_t shmid = shared_open(name, port);
    if (shmid != (key_t)-1) {
      struct shmid_ds buf;
      if(shmctl(shmid, IPC_STAT, &buf) < 0) {
	fprintf(stderr, "Could not close memory segment.\n");
      }
      fprintf(stderr, "raw: %zd\n", buf.shm_segsz);
      fprintf(stderr, "data offset: %zd\n", dataoffset);
      printf("%zd\n", buf.shm_segsz - dataoffset);
    }
  } else if (strcmp(command, "fullsize") == 0) {
    key_t shmid = shared_open(name, port);
    if (shmid != (key_t)-1) {
      struct shmid_ds buf;
      if(shmctl(shmid, IPC_STAT, &buf) < 0) {
	fprintf(stderr, "Could not close memory segment.\n");
      }
      size_t bytes = buf.shm_segsz;
      printf("%zd (B) %.1lf (MiB) %.1lf (GiB) %.1lf (TiB)\n", bytes, bytes*1.0/1024/1024, bytes*1.0/1024/1024/1024, bytes*1.0/1024/1024/1024/1024);
    }
  } else if (strcmp(command, "dd") == 0) {
    key_t shmid = shared_open(name, port);
    if (shmid != (key_t)-1) {
      fprintf(stderr, "shmid: %d\n", shmid);
      char *shm = shared_mem(shmid);
      if (!shm) {
	fprintf(stderr, "no shared memory exists\n");
	exit(EXIT_FAILURE);
      }

      struct shmid_ds buf;
      if(shmctl(shmid, IPC_STAT, &buf) < 0) {
	fprintf(stderr, "Could not close memory segment.\n");
      }

      // don'dump the superblock
      for (size_t o = 0 /*dataoffset*/; o < buf.shm_segsz; o+= 4096) {
	write(1, shm + o, 4096);
      }
    }
  } else if (strcmp(command, "work") == 0) {
    key_t shmid = shared_open(name, port);
    if (shmid != (key_t)-1) {
      // get the ram disk
      fprintf(stderr, "shmid: %d\n", shmid);
      volatile char *shm = shared_mem(shmid);
      if (!shm) {
	printf("no shared memory exists\n");
	exit(EXIT_FAILURE);
      }

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
      
      shm = shm + dataoffset;
      // go for it!
      
      size_t t = 0;
      //      sem_t *sem = (sem_t*)shm;
      volatile int *vp = (int*)(shm + sizeof(sem_t));
      
      while ( 1 ) {
	//	sem_wait(sem);
	
	const int v = *vp;
	*vp = v+1;
	assert(*vp == (v+1));

	//	sem_post(sem);

	t++;
	if ((t%1000)==0) {
	  printf("[%zd] %d %d\n", t++, shmid, (int)(*vp));
	}
	//	sleep(1);
      }
    }
  }
  
  free(name);
  free(nic);



  return 0;
}
