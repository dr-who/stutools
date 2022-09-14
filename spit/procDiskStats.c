
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <time.h>

#include "procDiskStats.h"



char *getFieldFromUdev(size_t major, size_t minor, char *match) {
  char s[100];
  sprintf(s, "/run/udev/data/b%zd:%zd", major, minor);
  FILE *fp = fopen(s, "rt");
  if (!fp) {
    perror(s);
    exit(1);
  }

  char *line = calloc(1000,1);
  char *result = calloc(1000,1);
  while (fgets(line, 1000, fp) != NULL) {
    if (strstr(line, match)) {
      sscanf(strstr(line, "=")+1, "%s", result);
      //      fprintf(stderr,"matched: %s\n", result);
    }
  }
  fclose(fp);
  free(line);

  return result;
}

void procDiskStatsInit(procDiskStatsType *d) {
  memset(d, 0, sizeof(procDiskStatsType));
}


void procDiskStatsSample(procDiskStatsType *d) {
  assert(d->num == 0);
  FILE *fp = fopen("/proc/diskstats", "rt");
  if (!fp) {
    perror("/proc/diskstats");
    exit(1);
  }
  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  char str[1000];
  while ((read = getline(&line, &len, fp)) != -1) {
    size_t col1,col2,col4,col5,col6,col7,col8,col9,col10,col11,col12,col13,col14;
    sscanf(line,"%zu %zu %s %zu %zu %zu %zu %zu %zu %zu %zu %zu %zu %zu", &col1, &col2, str, &col4, &col5, &col6, &col7, &col8, &col9, &col10, &col11, &col12, &col13, &col14);
    d->devices = realloc(d->devices, (d->num+1) * (sizeof(procDiskLineType)));
    //    fprintf(stderr,"==%zu\n", discardcount1);
    d->devices[d->num].majorNumber = col1;
    d->devices[d->num].minorNumber = col2;
    d->devices[d->num].deviceName = strdup(str);
    d->devices[d->num].readsCompleted = col4; //field1
    d->devices[d->num].readsMerged = col5;
    d->devices[d->num].sectorsRead = col6; 
    d->devices[d->num].timeSpentReading_ms = col7; //field4
    d->devices[d->num].writesCompleted = col8;
    d->devices[d->num].writesMerged = col9;
    d->devices[d->num].sectorsWritten = col10;
    d->devices[d->num].timeSpentWriting_ms = col11;
    d->devices[d->num].IOsCurrentInProgress = col12;
    d->devices[d->num].timeSpentDoingIO_ms = col13;
    d->devices[d->num].weightedTimeSpentDoingIOs_ms = col14;

    d->devices[d->num].serialShort = strdup(getFieldFromUdev(col1, col2, "E:ID_SERIAL="));
    d->devices[d->num].idVendor = strdup(getFieldFromUdev(col1, col2, "E:ID_VENDOR="));
    d->devices[d->num].idModel = strdup(getFieldFromUdev(col1, col2, "E:ID_MODEL="));
    
    d->num++;
  }
  free(line);
  fclose(fp);
}

void procDiskStatsFree(procDiskStatsType *d) {
  for (size_t i = 0; i < d->num; i++) {
    if (d->devices) {
      if (d->devices[i].deviceName)
	free(d->devices[i].deviceName);
      if (d->devices[i].serialShort)
	free(d->devices[i].serialShort);
      if (d->devices[i].idVendor)
	free(d->devices[i].idVendor);
      if (d->devices[i].idModel)
	free(d->devices[i].idModel);
    }
  }
  if (d->devices) 
    free(d->devices);
  d->num = 0;
}

void procDiskStatsDump(procDiskStatsType *d) {
  procDiskStatsDumpThres(d, 0);
}

void procDiskStatsDumpThres(procDiskStatsType *d, float msthres) {
  for (size_t i = 0; i < d->num; i++) {
    float r_ms = d->devices[i].timeSpentReading_ms * 1.0 / d->devices[i].readsCompleted;
    float w_ms = d->devices[i].timeSpentWriting_ms * 1.0 / d->devices[i].writesCompleted;
    
    if (d->devices[i].readsCompleted || d->devices[i].writesCompleted)
      if (r_ms > msthres || w_ms > msthres) {{
	  fprintf(stderr,"%ld\t%zd:%zd\t%s\tR %.1lf ms\t W %.1lf ms\t%zd ms\n", (long)time(NULL),
		  d->devices[i].majorNumber, d->devices[i].minorNumber, d->devices[i].deviceName,
		  /*		  d->devices[i].idModel,
		  d->devices[i].serialShort,
		  d->devices[i].idVendor,*/
		  r_ms,
		  w_ms,
		  d->devices[i].timeSpentDoingIO_ms
		  );
	}
    }
  }
}

void procDiskStatsCopy(procDiskStatsType *new, procDiskStatsType *old) {

  procDiskStatsInit(new);
  new->num = old->num;
  new->devices = calloc(new->num, sizeof(procDiskLineType));

  for (size_t i = 0; i < new->num; i++) {
    new->devices[i] = old->devices[i];
    new->devices[i].deviceName = strdup(old->devices[i].deviceName);
    new->devices[i].serialShort = strdup(old->devices[i].serialShort);
    new->devices[i].idVendor = strdup(old->devices[i].idVendor);
    new->devices[i].idModel = strdup(old->devices[i].idModel);
  }
}


procDiskStatsType procDiskStatsDelta(procDiskStatsType *old, procDiskStatsType *new) {
  procDiskStatsType ret;
  procDiskStatsInit(&ret);

  ret.num = new->num;
  ret.devices = calloc(old->num, sizeof(procDiskLineType));

  for (size_t i = 0; i < new->num; i++) {
    if (i < old->num) {
      if (strcmp(old->devices[i].deviceName, new->devices[i].deviceName) == 0) {
	ret.devices[i].majorNumber = new->devices[i].majorNumber;
	ret.devices[i].minorNumber = new->devices[i].minorNumber;

	// copy strings
	ret.devices[i].deviceName = strdup(new->devices[i].deviceName);
	ret.devices[i].serialShort = strdup(new->devices[i].serialShort);
	ret.devices[i].idVendor = strdup(new->devices[i].idVendor);
	ret.devices[i].idModel = strdup(new->devices[i].idModel);
	
	ret.devices[i].readsCompleted = new->devices[i].readsCompleted - old->devices[i].readsCompleted;
	ret.devices[i].writesCompleted = new->devices[i].writesCompleted - old->devices[i].writesCompleted;
	
	ret.devices[i].timeSpentWriting_ms = new->devices[i].timeSpentWriting_ms - old->devices[i].timeSpentWriting_ms;
	ret.devices[i].timeSpentReading_ms = new->devices[i].timeSpentReading_ms - old->devices[i].timeSpentReading_ms;

	ret.devices[i].sectorsWritten = new->devices[i].sectorsWritten - old->devices[i].sectorsWritten;
	ret.devices[i].sectorsRead = new ->devices[i].sectorsRead - old->devices[i].sectorsRead;

	ret.devices[i].timeSpentDoingIO_ms = new->devices[i].timeSpentDoingIO_ms - old->devices[i].timeSpentDoingIO_ms;
      }
    }
  }

  return ret;
}
