#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <fcntl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <signal.h>

/**
 *
 */
#include <stdlib.h>
#include <stdio.h>
#include <getopt.h>
#include <string.h>



void generateMakefile(const char *device, const char *filename) {
  FILE *fp = fopen(filename, "wt");
  if (!fp) {perror(filename); exit(1); }

  fprintf(fp, "DEVICE=%s\n", device);
  fprintf(fp, "SPIT=../spit/spit\n");
  fprintf(fp, "DIST=../utils/dist\n");
  fprintf(fp, "MEDIAN=../utils/median\n");
  fprintf(fp, "CACHE=16\n");
  fprintf(fp, "\n");
  fprintf(fp, "shorttime=10\n");
  fprintf(fp, "longtime=30\n");
  fprintf(fp, "voltime=100\n");
  fprintf(fp, "\n");
  fprintf(fp, "all: setupdirs reportshort reportlong\n");
  fprintf(fp, "\n");
  fprintf(fp, "help:\n");
  fprintf(fp, "\techo \"make -F Makefile.r1 reportshort or reportlong\"\n");
  fprintf(fp, "\n");
  fprintf(fp, "clean:\n");
  fprintf(fp, "\t/bin/rm -f reportshort/* reportlong/*\n");
  fprintf(fp, "\n");
  fprintf(fp, "setupdirs:\n");
  fprintf(fp, "\tmkdir -p reportshort reportlong report\n");
  fprintf(fp, "\n");
  fprintf(fp, "copyshorttoresult:\n");
  fprintf(fp, "\tcp -f reportshort/* report/\n");
  fprintf(fp, "\n");
  fprintf(fp, "copylongtoresult:\n");
  fprintf(fp, "\tcp -f reportlong/* report/\n");
  fprintf(fp, "\n");
  
  char *str[2] = {"short","long"};
  char *rounds[7]={"4","8","16","32","64","256","1024"};
  for (size_t i = 0; i < 2; i++) {
    fprintf(fp, "ssconvergency%s-iops.gnu: report%s\n", str[i], str[i]);
    fprintf(fp, "\techo \"set style data linespoints\" > $@\n");
    fprintf(fp, "\techo \"set title 'Steady State Convergence Plot (IO/s): $(DEVICE)'\" >> $@\n");
    fprintf(fp, "\techo \"set xlabel 'rounds'\" >> $@\n");
    fprintf(fp, "\techo \"set ylabel 'IO/s'\" >> $@\n");
    fprintf(fp, "\techo \"set key outside\" >> $@\n");
    fprintf(fp, "\techo \"set grid\" >> $@\n");
    fprintf(fp, "\techo \"plot [][0:] ");
    for (size_t r = 0; r < 7; r++) {
      if (r > 0) fprintf(fp, ",");
      fprintf(fp, "'report%s/B%s.iops' title '%s'", str[i], rounds[r], rounds[r]);
    }
    fprintf(fp, "\" >> $@ \n");
    fprintf(fp, "\n");

    fprintf(fp, "ssconvergency%s-mbps.gnu: report%s\n", str[i], str[i]);
    fprintf(fp, "\techo \"set style data linespoints\" > $@\n");
    fprintf(fp, "\techo \"set title 'Steady State Convergence Plot (MB/s): $(DEVICE)'\" >> $@\n");
    fprintf(fp, "\techo \"set xlabel 'rounds'\" >> $@\n");
    fprintf(fp, "\techo \"set ylabel 'MB/s'\" >> $@\n");
    fprintf(fp, "\techo \"set key outside\" >> $@\n");
    fprintf(fp, "\techo \"set grid\" >> $@\n");
    fprintf(fp, "\techo \"plot [][0:] ");
    for (size_t r = 0; r < 7; r++) {
      if (r > 0) fprintf(fp, ",");
      fprintf(fp, "'report%s/B%s.mbps' title '%s'", str[i], rounds[r], rounds[r]);
    }
    fprintf(fp, "\" >> $@ \n");
    fprintf(fp, "\n");



    fprintf(fp, "ssconvergency%s-vol.gnu: report%s/vol\n", str[i], str[i]);
    fprintf(fp, "\techo \"set style data linespoints\" > $@\n");
    fprintf(fp, "\techo \"set title 'Volatility Plot (MB/s): $(DEVICE)'\" >> $@\n");
    fprintf(fp, "\techo \"set xlabel 'time (s)'\" >> $@\n");
    fprintf(fp, "\techo \"set ylabel 'MB/s'\" >> $@\n");
    fprintf(fp, "\techo \"set key outside\" >> $@\n");
    fprintf(fp, "\techo \"set grid\" >> $@\n");
    fprintf(fp, "\techo \"plot [][0:] ");
    char *vs[9]={"100%","99.85%","97.5%","84%","50%","16%","2.5%","0.15%","0%"};
    for (size_t r = 0; r < 9; r++) {
      if (r > 0) fprintf(fp, ",");
      fprintf(fp, "'report%s/vol' using %zd title '%s'", str[i], r+1, vs[r]);
    }
    fprintf(fp, ",'report%s/vol' using 10 title 'Mean'", str[i]);
    fprintf(fp, "\" >> $@ \n");
    fprintf(fp, "\n");


    fprintf(fp, "report%s: report%s/all copy%storesult\n", str[i], str[i], str[i]);
    fprintf(fp, "\n");
    fprintf(fp, "# %s\n", str[i]);
    fprintf(fp, "\n");
    fprintf(fp, "report%s/all: report%s/vol ", str[i], str[i]);
    for (size_t r = 0; r < 7; r++) {
      fprintf(fp, "report%s/B%s.iops report%s/B%s.mbps ", str[i], rounds[r], str[i], rounds[r]);
    }
    fprintf(fp,"\n");
    fprintf(fp, "\tcat $^ > $@\n");
    fprintf(fp, "\n");


    for (size_t r = 0; r < 7; r++) {
      fprintf(fp, "\nreport%s/B%s.iops: ", str[i], rounds[r]);
      fprintf(fp, "report%s/B%s.rounds\n", str[i], rounds[r]);
      fprintf(fp,"\tawk '{print $$6}' $<.1 | $(MEDIAN) | tail -n 1 > $@\n");
      for (size_t j = 2; j <= 7; j++) {
	fprintf(fp,"\tawk '{print $$6}' $<.%zd | $(MEDIAN) | tail -n 1 >> $@\n", j);
      }
      fprintf(fp,"\n");
    }
    fprintf(fp,"\n");
    
    for (size_t r = 0; r < 7; r++) {
      fprintf(fp, "\nreport%s/B%s.mbps: ", str[i], rounds[r]);
      fprintf(fp, "report%s/B%s.rounds\n", str[i], rounds[r]);
      fprintf(fp,"\tawk '{print $$5}' $<.1 | $(MEDIAN) | tail -n 1 > $@\n");
      for (size_t j = 2; j <= 7; j++) {
	fprintf(fp,"\tawk '{print $$5}' $<.%zd | $(MEDIAN) | tail -n 1 >> $@\n", j);
      }
      fprintf(fp,"\n");
    }
    fprintf(fp,"\n");


    fprintf(fp,"\nreport%s/vol: \n", str[i]);
    fprintf(fp,"\t$(SPIT) -t $(voltime)  -f $(DEVICE)  -c zws1k256 -B $@.temp -i $(CACHE)\n");
    fprintf(fp,"\tawk '{print $$5}' $@.temp | $(DIST) -n -a 30 > $@\n");
    fprintf(fp,"\n");
    
    for (size_t r = 0; r < 7; r++) {
      fprintf(fp, "\nreport%s/B%s.rounds:\n", str[i], rounds[r]);
      for (size_t j = 1; j <= 7; j++) {
	fprintf(fp, "\t$(SPIT) -t $(%stime)  -f $(DEVICE)  -c ws0k%s -B $@.%zd -i $(CACHE)\n", str[i], rounds[r], j);
      }
      fprintf(fp, "\tcat $@.* > $@\n");
      fprintf(fp, "\n");
    }
    fprintf(fp, "\n");
    fprintf(fp, "\n");
    fprintf(fp, "\n");
    fprintf(fp, "\n");
    fprintf(fp, "\n");
    fprintf(fp, "\n");
  }

}




/**
 * main
 *
 */
int main(int argc, char *argv[]) {
  if (argc <= 1) {
    fprintf(stderr,"*usage* ./spitreport -f device -I subdevices.txt -r reportname\n");
    fprintf(stderr,"\nGenerates Makefile.reportname\n");
    fprintf(stderr,"\nUsage:\n");
    fprintf(stderr,"\t$ ./spitreport -f /dev/test-lv -I devices.txt -r v1report\n");
    fprintf(stderr,"\t$ make -f Makefile.v1report\n");
    exit(1);
  }

  int opt = 0;
  char *device = NULL, *subdevices = NULL, *reportname = NULL;
  while ((opt = getopt(argc, argv, "f:I:r:")) != -1) {
    switch (opt) {
    case 'f': device = strdup(optarg); break;
    case 'I': subdevices = strdup(optarg); break;
    case 'r': reportname = strdup(optarg); break;
    default:
      break;
    }
  }

  fprintf(stderr,"*info* %s, %s, %s\n", device, subdevices, reportname);

  generateMakefile(device, reportname);

  exit(0);
}
  
  
