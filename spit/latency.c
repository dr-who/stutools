#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <math.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>

#include <assert.h>
#include <pthread.h>

#include "latency.h"
#include "positions.h"


void latencyClear(latencyType *lat) {
  histSetup(&lat->histRead, 0, 10000, 1e-2);
  histSetup(&lat->histWrite, 0, 10000, 1e-2);
}  

void latencySetup(latencyType *lat, positionContainer *pc) {
  
  for (int i = 0; i < (int) pc->sz; i++) if (pc->positions[i].finishTime>0) {
    if (pc->positions[i].action == 'R')
      histAdd(&lat->histRead, 1000 * (pc->positions[i].finishTime - pc->positions[i].submitTime));
    else if (pc->positions[i].action == 'W')
      histAdd(&lat->histWrite, 1000 * (pc->positions[i].finishTime - pc->positions[i].submitTime));
  }
}
void latencySetupSizeonly(latencyType *lat, positionContainer *pc, size_t size) {
  
  for (int i = 0; i < (int) pc->sz; i++)
    if (pc->positions[i].finishTime>0)
      if (pc->positions[i].len == size) {
	if (pc->positions[i].action == 'R')
	  histAdd(&lat->histRead, 1000 * (pc->positions[i].finishTime - pc->positions[i].submitTime));
	else if (pc->positions[i].action == 'W')
	  histAdd(&lat->histWrite, 1000 * (pc->positions[i].finishTime - pc->positions[i].submitTime));
      }
}



void latencyOverTime(positionContainer *origpc) {
  if (origpc) {
    FILE *fp = fopen("time_vs_latency.gnu", "wt");
    if (!fp) {perror("time_vs_latency.gnu");exit(1);}
    //plot '<sort -k 12n bob' using ($13-$12) with lines
    fprintf(fp, "set title 'Time vs. Latency (n=%zd)\n", origpc->sz);
    fprintf(fp, "set xlabel 'Time (seconds)'\n");
    fprintf(fp, "set ylabel 'Latency (seconds)'\n");
    fprintf(fp, "set key top center\n");
    fprintf(fp, "set grid\n");
    fprintf(fp, "plot '<sort -k 1n t_vs_lat.txt | grep -w W' using 1:2 with lines title 'Latency over time (writes)', '<sort -k 1n t_vs_lat.txt | grep -w R' using 1:2 with lines title 'Latency over time (reads)'\n");
    fclose(fp);
    fp = fopen("t_vs_lat.txt", "wt");
    if (!fp) {perror("t_vs_lat.txt"); exit(1);}
    double lowesttime = 9e9;
    for (size_t i = 0; i <origpc->sz; i++) {
      positionType *p = &origpc->positions[i];
      if (p->finishTime > 0) {
	if (p->submitTime < lowesttime) lowesttime = p->submitTime;
      }
    }
    
    for (size_t i = 0; i <origpc->sz; i++) {
      positionType *p = &origpc->positions[i];
      if (p->finishTime > 0) {
	fprintf(fp, "%lf %lf %c %d\n", p->submitTime - lowesttime, p->finishTime - p->submitTime, p->action, p->len);
      }
    }
    fclose(fp);
  }
}


void latencyLenVsLatency(positionContainer *origpc, int num) {
  FILE *fp_r = fopen("size_vs_latency_r.txt", "wt");
  FILE *fp_w = fopen("size_vs_latency_w.txt", "wt");
  assert(fp_r);
  assert(fp_w);
  const double jitter = 10.0 / 100.0; // 10%
  srand48(0);
  size_t count = 0 ;
  for (int n = 0; n < num; n++) {
    for (int i = 0; i < (int) origpc[n].sz; i++) if (origpc[n].positions[i].finishTime > 0) {
	if (origpc[n].positions[i].action == 'R') {
	  count++;
	  double v = 1 + (drand48() * 2*jitter) - jitter;
	  fprintf(fp_r, "%.6lf\t%.6lf\n", origpc[n].positions[i].len * v, origpc[n].positions[i].finishTime - origpc[n].positions[i].submitTime);
	} else if (origpc[n].positions[i].action == 'W') {
	  count++;
	  double v = 1 + (drand48() * 2*jitter) - jitter;
	  fprintf(fp_w, "%.6lf\t%.6lf\n", origpc[n].positions[i].len * v, origpc[n].positions[i].finishTime - origpc[n].positions[i].submitTime);
	} 
      }
  }
  fclose(fp_r);
  fclose(fp_w);

  FILE *fp = fopen("size_vs_latency.gnu", "wt");
  if (fp) {
    const char *type = count < 100000 ? "points" : "dots";
    
    fprintf(fp, "set key above\n");
    fprintf(fp, "set title 'Block size vs Latency (%.0lf%% horiz jitter, n=%zd)\n", jitter*100.0, count);
    fprintf(fp, "set log x\n");
    fprintf(fp, "set log y\n");
    fprintf(fp, "set xtics auto\n");
    fprintf(fp, "set logscale x 2\n");
    fprintf(fp, "set format x '2^{%%L}'\n");
    fprintf(fp, "set grid\n");
    fprintf(fp, "set xlabel 'Block size (bytes)'\n");
    fprintf(fp, "set ylabel 'Latency (s)'\n");
    fprintf(fp, "plot 'size_vs_latency_r.txt' using 1:2:2 with %s palette title 'Block reads', 'size_vs_latency_w.txt' using 1:2:2 with %s palette title 'Block writes'\n", type, type);
  } else {
    perror("filename");
  }
  fclose(fp);
}
  
      
  

void latencyStats(latencyType *lat) {
  double median, three9, four9, five9;
  if (histCount(&lat->histRead)) {
    histSumPercentages(&lat->histRead, &median, &three9, &four9, &five9, 1);
    const size_t n = histCount(&lat->histRead);
    if (n<10000) three9 = NAN; // to report 1 in 1,000 values. Try and get 10 of them.
    if (n<100000) four9 = NAN;
    if (n<1000000) five9 = NAN;
    fprintf(stderr,"*info* read latency: n = %zd, mean = %.3lf ms (sd = %.2lf ms), median = %.2lf ms, 99.9%% <= %.2lf ms, 99.99%% <= %.2lf ms, 99.999%% <= %.2lf ms\n", n, histMean(&lat->histRead), nlSD(&lat->histRead.nl), median, three9, four9, five9);
    histSave(&lat->histRead, "spit-latency-histogram-read.txt", 1);
  }
  if (histCount(&lat->histWrite)) {
    histSumPercentages(&lat->histWrite, &median, &three9, &four9, &five9, 1);
    const size_t n = histCount(&lat->histWrite);
    if (n<10000) three9 = NAN;
    if (n<100000) four9 = NAN;
    if (n<1000000) five9 = NAN;
    fprintf(stderr,"*info* write latency: n = %zd, mean = %.3lf ms (sd = %.2lf ms), median = %.2lf ms, 99.9%% <= %.2lf ms, 99.99%% <= %.2lf ms, 99.999%% <= %.2lf ms\n", n, histMean(&lat->histWrite), nlSD(&lat->histWrite.nl), median, three9, four9, five9);
    histSave(&lat->histWrite, "spit-latency-histogram-write.txt", 1);
  }
}


void latencyFree(latencyType *lat) {
  histFree(&lat->histRead);
  histFree(&lat->histWrite);
}


void latencyReadGnuplot(latencyType *lat) {
  double median, three9, four9, five9;
  if (histCount(&lat->histRead)) {
    histSumPercentages(&lat->histRead, &median, &three9, &four9, &five9, 1);
  }
  FILE *fp = fopen("spit-latency-histogram-read.gnu", "wt");
  if (fp) {
    fprintf(fp, "set key above\n");
    fprintf(fp, "set title 'Response Time Histogram - Confidence Level Plot (n=%zd)'\n", lat->histRead.dataCount);
    fprintf(fp, "set nonlinear x via log10(x)/log10(2) inverse 2**x\n"); // xtics like 1, 2, 4, 8 ms
    fprintf(fp, "set xtics 2 logscale out\n");
    fprintf(fp, "set log x\n");
    fprintf(fp, "set log y\n");
    fprintf(fp, "set yrange [0.9:%lf]\n", histMaxCount(&lat->histRead)*1.1);
    fprintf(fp, "set y2tics 10 out\n");
    fprintf(fp, "set grid\n");
    fprintf(fp, "set xrange [%lf:%lf]\n", histLowestPresentValue(&lat->histRead)*0.95, histHighestPresentValue(&lat->histRead)*1.05);
    fprintf(fp, "set y2range [0:100]\n");
    fprintf(fp, "set xlabel 'Time (ms) - 0.01ms bins'\n");
    fprintf(fp, "set ylabel 'Count'\n");
    fprintf(fp, "set y2label 'Confidence level'\n");
    fprintf(fp, "plot 'spit-latency-histogram-read.txt' using 1:2 with imp title 'Read Latency', 'spit-latency-histogram-read.txt' using 1:3 with lines title '%% Confidence' axes x1y2,'<echo %lf 100000' with imp title 'ART=%.3lf' axes x1y2, '<echo %lf 100000' with imp title '99.9%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.99%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.999%%=%.2lf' axes x1y2\n", median, median, three9, three9, four9, four9, five9, five9);
  } else {
    perror("filename");
  }
  if (fp) {
    fclose(fp);
    fp = NULL;
  }
}


void latencyWriteGnuplot(latencyType *lat) {
  double median = 0, three9 = 0, four9 = 0, five9 = 0;
  if (histCount(&lat->histWrite)) {
    histSumPercentages(&lat->histWrite, &median, &three9, &four9, &five9, 1);
  }
  FILE *fp = fopen("spit-latency-histogram-write.gnu", "wt");
  if (fp) {
    fprintf(fp, "set key above\n");
    fprintf(fp, "set title 'Response Time Histogram - Confidence Level Plot (n=%zd)'\n", lat->histWrite.dataCount);
    fprintf(fp, "set nonlinear x via log10(x)/log10(2) inverse 2**x\n"); // xtics like 1, 2, 4, 8 ms
    fprintf(fp, "set xtics 2 logscale out\n");
    fprintf(fp, "set log x\n");
    fprintf(fp, "set log y\n");
    fprintf(fp, "set yrange [0.9:%lf]\n", histMaxCount(&lat->histWrite)*1.1);
    fprintf(fp, "set y2tics 10 out\n");
    fprintf(fp, "set grid\n");
    fprintf(fp, "set xrange [%lf:%lf]\n", histLowestPresentValue(&lat->histRead)*0.95, histHighestPresentValue(&lat->histRead)*1.05);
    fprintf(fp, "set y2range [0:100]\n");
    fprintf(fp, "set xlabel 'Time (ms) - 0.01ms bins'\n");
    fprintf(fp, "set ylabel 'Count'\n");
    fprintf(fp, "set y2label 'Confidence level'\n");
    fprintf(fp, "plot 'spit-latency-histogram-write.txt' using 1:2 with imp title 'Write Latency', 'spit-latency-histogram-write.txt' using 1:3 with lines title '%% Confidence' axes x1y2,'<echo %lf 100000' with imp title 'ART=%.3lf' axes x1y2, '<echo %lf 100000' with imp title '99.9%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.99%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.999%%=%.2lf' axes x1y2\n", median, median, three9, three9, four9, four9, five9, five9);
  } else {
    perror("filename");
  }
  if (fp) {
    fclose(fp);
    fp = NULL;
  }
}
