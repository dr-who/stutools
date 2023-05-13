#include <stdio.h>
#include <unistd.h>

#include <time.h>

#include <readline/readline.h>
#include <readline/history.h>

#include "utils.h"

int keepRunning = 1;

int com_status();

typedef struct {
  char *name;			/* User printable name of the function. */
  char *doc;			/* Documentation for this function.  */
} COMMAND;

COMMAND commands[] = {
  { "status", "Show system status" },
};

const char *BOLD="\033[1m";
const char *RED="\033[31m";
const char *GREEN="\033[32m";
const char *END="\033[0m";


void header(const char *hostname, const int tty) {
  time_t t = time(NULL);
  struct tm tm = *localtime(&t);

  if (tty) {
    fprintf(stdout, "%s", BOLD);
  }
  fprintf(stdout,"stush (secure sandpit: v0.1) - %s%s - %s", hostname, tty?" (interactive)":"", asctime(&tm));
  if (tty) {
    fprintf(stdout, "%s", END);
  }
}

void colour_printNumber(const double value, const unsigned int good, const char *suffix, const int tty) {
  if (tty) {
    if (good) {
      fprintf(stdout, "%s", GREEN);
    } else {
      fprintf(stdout, "%s", RED);
    }
  }

  fprintf(stdout, "%.0lf", value);

  fprintf(stdout, "%s", suffix);
  if (tty) {
    fprintf(stdout, "%s", END);
  }
}

void colour_printString(const char *string, const unsigned int good, const char *suffix, const int tty) {
  if (tty) {
    if (good) {
      fprintf(stdout, "%s", GREEN);
    } else {
      fprintf(stdout, "%s", RED);
    }
  }

  fprintf(stdout, "%s", string);

  fprintf(stdout, "%s", suffix);
  if (tty) {
    fprintf(stdout, "%s", END);
  }
}

  


void cmd_status(const int tty) {
  char *os = OSRelease();
  fprintf(stdout,"OS Release:       ");
  colour_printString(os, 1, "\n", tty);
  if (os) free(os);
  
  fprintf(stdout,"Uptime (days):    %.0lf\n", getUptime()/86400.0);
  fprintf(stdout,"Load average:     %.1lf\n", loadAverage());
  fprintf(stdout,"Total RAM:        %.0lf GiB\n", TOGiB(totalRAM()));
  fprintf(stdout,"Free RAM:         ");
  colour_printNumber(TOGiB(freeRAM()), TOGiB(freeRAM()) >= 1, " GiB\n", tty);
  
  fprintf(stdout,"Swap Total:       %.0lf GB\n", TOGB(swapTotal()));
  char *cpu = getCPUModel();
  fprintf(stdout,"CPU Model:        %s\n", cpu);
  if (cpu) free(cpu);
  
  fprintf(stdout,"NUMA:             %d\n", getNumaCount());
  fprintf(stdout,"CPUs per NUMA     %d\n", cpuCountPerNuma(0));

  char *power = getPowerMode();
  fprintf(stdout,"Power mode:       ");
  colour_printString(power, (strcmp("performance", power)==0), "\n", tty);
  if (power) free(power);
}


int main() {
  char hostname[91], prefix[100];
  if (gethostname(hostname, 90)) {
    sprintf(hostname, "stush");
  }

  int tty = isatty(1);
  
  header(hostname, tty);
  cmd_status(tty);

  sprintf(prefix, "%s$ ", hostname);


  while (1) {
    char *line = readline (prefix);
    
    if (line == NULL) {
      break;
    }

    
    add_history (line);
    int known = 0;
    for (size_t i = 0; i < sizeof(commands)/sizeof(COMMAND); i++) {
      if (commands[i].name && (strncmp(line, commands[i].name, strlen(commands[i].name)) == 0)) {
	if (strcmp(commands[i].name, "status") == 0) {
	  cmd_status(tty);
	}
	known = 1;
	break;
      }
    }
    if (!known) {
      fprintf(stdout,"%s: unknown command\n", line);
    }
  }
  fprintf(stdout,"\n");

  return 0;
}
