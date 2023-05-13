#define _GNU_SOURCE

#include <stdio.h>
#include <unistd.h>

#include <time.h>
#include <signal.h>
#include <locale.h>

#include <readline/readline.h>
#include <readline/history.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include <sys/file.h>
          

#include "utils.h"
#include "procDiskStats.h"

int keepRunning = 1;
int TeReo = 0;

void intHandler(int d)
{
  if (d) {}
  //  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}


int com_status();

typedef struct {
  char *name;			/* User printable name of the function. */
  char *doc;			/* Documentation for this function.  */
} COMMAND;

COMMAND commands[] = {
  { "status", "Show system status" },
  { "pwgen", "Generate cryptographically complex 200-bit random password"},
  { "lsblk", "List drive block devices"},
  { "readspeed", "Measure read speed on device (readspeed /dev/sda)"},
  { "exit", "Exit the secure shell (or ^d/EOF)"}
};

const char *BOLD="\033[1m";
const char *RED="\033[31m";
const char *GREEN="\033[32m";
const char *END="\033[0m";


void header(const int tty) {
  time_t t = time(NULL);
  struct tm tm = *localtime(&t);

  char *lang = getenv("LANG");
  if (lang == NULL) {
    lang = strdup("en_US");
  } else {
    lang = strdup(lang);
  }

  setlocale(LC_ALL, lang);

  if (tty) {
    printf("%s", BOLD);
  }
  char timestring[1000];
  strftime(timestring, 999, "%c\n", &tm);
  printf("stush: (secure sandpit: v0.1) - %s\n", timestring);
  if (tty) {
    printf("%s", END);
  }

  if (strncasecmp(lang, "mi_NZ", 5) == 0) {
    if (tty) printf("%s", BOLD);
    TeReo = 1;
    printf("Kia ora (%s)\n", lang);
    if (tty) printf("%s", END);
  }
  fflush(stdout);
  
  free(lang);
}

void colour_printNumber(const double value, const unsigned int good, const char *suffix, const int tty) {
  if (tty) {
    if (good) {
      printf("%s", GREEN);
    } else {
      printf("%s", RED);
    }
  }

  printf("%.0lf", value);

  printf("%s", suffix);
  if (tty) {
    printf("%s", END);
  }
}

void colour_printString(const char *string, const unsigned int good, const char *suffix, const int tty) {
  if (tty) {
    if (good) {
      printf("%s", GREEN);
    } else {
      printf("%s", RED);
    }
  }

  printf("%s", string);

  printf("%s", suffix);
  if (tty) {
    printf("%s", END);
  }
}

void cmd_listAll() {
  printf("Commands: \n");
  for (size_t i = 0; i < sizeof(commands)/sizeof(commands[0]); i++) {
    printf("  %-10s \t %s\n", commands[i].name, commands[i].doc);
  }
}


void cmd_pwgen(int tty) {
  const size_t len = 32;
  double entropy = 0;
  unsigned char *pw = NULL;
  do {
    if (pw) {
      free(pw);
    }
    
    pw = passwordGenerate(len);
    entropy = entropyTotalBits(pw, len, 1);
  } while (entropy < 200 && keepRunning);

  printf("%s ", pw);
  char ss[1000];
  sprintf(ss, "(%.1lf bits of entropy)", entropy);
  colour_printString(ss, entropy >= 200, "\n", tty);
  
  free(pw);
}


size_t countDriveBlockDevices() {
  procDiskStatsType d;
  procDiskStatsInit(&d);
  procDiskStatsSample(&d);
  size_t count = 0;
  for (size_t i = 0; i < d.num; i++) {
    if (d.devices[i].majorNumber == 8) {
      int len = strlen(d.devices[i].deviceName);
      char lastchar = d.devices[i].deviceName[len-1];
      //      printf("%s %c\n", d.devices[i].deviceName, lastchar);
      if (!isdigit(lastchar))
	count++;
    }
  }
  return count;
}


void cmd_listDriveBlockDevices(int tty) {
  if (tty) {}
  
  procDiskStatsType d;
  procDiskStatsInit(&d);
  procDiskStatsSample(&d);

  if (tty) printf("%s", BOLD);
  printf("device   \tmaj:min\tGB\tvendor\t%-18s\n", "model");
  if (tty) printf("%s", END);

  for (size_t i = 0; i < d.num; i++) {
    if (d.devices[i].majorNumber == 8) {
      char path[1000];
      sprintf(path, "/dev/%s", d.devices[i].deviceName);
      size_t bdsize = blockDeviceSize(path);
      printf("/dev/%s\t%zd:%zd\t%.0lf\t%s\t%-18s\n", d.devices[i].deviceName, d.devices[i].majorNumber, d.devices[i].minorNumber, TOGB(bdsize), d.devices[i].idVendor, d.devices[i].idModel);
    }
  }
  
  procDiskStatsFree(&d);
}
  


void cmd_status(const char *hostname, const int tty) {
  char *os = OSRelease();
  printf("Hostname:          ");
  colour_printString(hostname, 1, "\n", tty);
  
  
  printf("OS Release:        ");
  colour_printString(os, 1, "\n", tty);
  if (os) free(os);
  
  printf("Uptime (days):     %.0lf\n", getUptime()/86400.0);
  printf("Load average:      %.1lf\n", loadAverage());
  printf("Total RAM:         %.0lf GiB\n", TOGiB(totalRAM()));
  printf("Free RAM:          ");
  colour_printNumber(TOGiB(freeRAM()), TOGiB(freeRAM()) >= 1, " GiB\n", tty);
  
  printf("Swap Total:        %.0lf GB\n", TOGB(swapTotal()));
  char *cpu = getCPUModel();
  printf("CPU Model:         %s\n", cpu);
  if (cpu) free(cpu);
  
  printf("NUMA:              %d\n", getNumaCount());
  printf("CPUs per NUMA      %d\n", cpuCountPerNuma(0));

  char *power = getPowerMode();
  printf("Power mode:        ");
  colour_printString(power, (strcmp("performance", power)==0), "\n", tty);
  if (power) free(power);

  printf("Entropy Available: ");
  int entropy = entropyAvailable();
  colour_printNumber(entropy, entropy > 200, " bits\n", tty);

  printf("Drive devices:     ");
  size_t drives = countDriveBlockDevices();
  colour_printNumber(drives, drives > 0, " devices\n", tty);
  
}


int main() {
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  

  char hostname[91], prefix[100];
  if (gethostname(hostname, 90)) {
    sprintf(hostname, "stush");
  }

  int tty = isatty(1);
  
  header(tty);
  cmd_status(hostname, tty);

  sprintf(prefix, "%s$ ", hostname);

  char *line = NULL;
  
  while (1) {
    keepRunning = 1;
    line = readline (prefix);
    
    if ((line == NULL) || (strcmp(line, "exit")==0) || (strcmp(line, "quit")==0)) {
      break;
    }
    if (strlen(line) < 1) {
      free(line);
      continue;
    }
      

    
    add_history (line);
    int known = 0;
    for (size_t i = 0; i < sizeof(commands)/sizeof(COMMAND); i++) {
      if (commands[i].name && (strncmp(line, commands[i].name, strlen(commands[i].name)) == 0)) {
	if (strcmp(commands[i].name, "status") == 0) {
	  cmd_status(hostname, tty);
	} else if (strcmp(commands[i].name, "pwgen") == 0) {
	  cmd_pwgen(tty);
	} else if (strcmp(commands[i].name, "lsblk") == 0) {
	  cmd_listDriveBlockDevices(tty);
	} else if (strcmp(commands[i].name, "readspeed") == 0) {
	  char *second = strchr(line, ' ');
	  if (second) {
	    int len = strlen(second+1);
	    if (len > 0) {
	      second++;

	      int fd = open(second, O_RDONLY | O_DIRECT);
	      if (fd < 0) {
		if (tty) printf("%s", BOLD);
		fprintf(stdout, "*error* the device '%s' couldn't be opened O_DIRECT\n", second);
		if (tty) printf("%s", END);
	      } else {
		unsigned int major, minor;
		majorAndMinor(fd, &major, &minor);
		if (major != 8) {
		  if (tty) printf("%s", BOLD);
		  fprintf(stdout, "*error* not a major 8 device\n");
		  if (tty) printf("%s", END);
		} else {
		  if (tty) printf("%s", BOLD);
		  fprintf(stdout,"*info* readspeed '%s', size=2 MiB for 5 seconds (MB/s)\n", second);
		  if (tty) printf("%s", END);
		  readSpeed(fd, 5, 2L*1024*1024);

		  if (tty) printf("%s", BOLD);
		  fprintf(stdout,"*info* readspeed '%s', size=4096 B for 5 seconds (MB/s)\n", second);
		  if (tty) printf("%s", END);
		  readSpeed(fd, 5, 4*1024);
		  close(fd);
		}
	      }
	    }
	  }
	}
	known = 1;
	break;
      }
    }
    if ((strcmp(line, "?")==0) || (strcmp(line, "help") == 0)) {
      cmd_listAll();
    } else {
      if (!known) {
	printf("%s: unknown command\n", line);
      }
    }
  }

  if (line == NULL) printf("\n");

  if (TeReo) {
    if (tty) printf("%s", BOLD);
    printf("Kia pai tō rā\n");
    if (tty) printf("%s", END);
  }
  
  free(line);

  return 0;
}
