#define _GNU_SOURCE

#include <stdio.h>
#include <unistd.h>
#include <string.h>

#include <time.h>
#include <signal.h>
#include <locale.h>

#include <readline/readline.h>
#include <readline/history.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include <sys/file.h>


#include <sys/types.h>
#include <sys/socket.h>
#include <net/if.h>


#include <netinet/in.h>
#include <arpa/inet.h>

#include <netdb.h>
#include <ifaddrs.h>
#include <linux/if_link.h>


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
  { "entropy", "Calc entropy of a string"},
  { "lsblk", "List drive block devices"},
  { "lsnic", "List IP/HW addresses"},
  { "pwgen", "Generate cryptographically complex 200-bit random password"},
  { "readspeed", "Measure read speed on device (readspeed /dev/sda)"},
  { "status", "Show system status" },
  { "exit", "Exit the secure shell (or ^d/EOF)"}
};

const char *BOLD="\033[1m";
const char *RED="\033[31m";
const char *GREEN="\033[32m";
const char *END="\033[0m";


void header(const int tty) {
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
  printf("stush: (secure sandpit: v0.2)\n\n");
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

void cmd_calcEntropy(const int tty, char *origstring) {
  char *string = strdup(origstring);
  const char *delim = " ";
  char *first = strtok(string, delim);
  if (first) {
    char *second = strtok(NULL, delim);
    if (second) {
      second = origstring + (second - string);
      double entropy = entropyTotalBits((unsigned char*)second, strlen(second), 1);
      printf("%s ", second);
      char ss[1000];
      sprintf(ss,"(%.1lf bits of entropy)", entropy);
      colour_printString(ss, entropy >= 200, "\n", tty);
    }
  }
  free(string);
}

  


void cmd_printHWAddr(char *nic) {

  int s;
  struct ifreq buffer;
  
  s = socket(PF_INET, SOCK_DGRAM, 0);
  memset(&buffer, 0x00, sizeof(buffer));
  strcpy(buffer.ifr_name, nic);
  ioctl(s, SIOCGIFHWADDR, &buffer);
  close(s);
  
  for( s = 0; s < 6; s++ ) {
      if (s > 0) printf(":");
      printf("%.2x", (unsigned char)buffer.ifr_hwaddr.sa_data[s]);
    }
  
  //  printf("\n");

  /*  s = socket(PF_INET, SOCK_DGRAM, 0);
  memset(&buffer, 0x00, sizeof(buffer));
  strcpy(buffer.ifr_name, nic);
  ioctl(s, SIOCGIFADDR, &buffer);
  close(s);

  printf("%s\n", inet_ntoa(((struct sockaddr_in *)&buffer.ifr_addr)->sin_addr));*/

}


// from getifaddrs man page
void cmd_listNICs(int tty) {
  if (tty) {}
  
  struct ifaddrs *ifaddr;
  int family, s;
  char host[NI_MAXHOST];
  
  if (getifaddrs(&ifaddr) == -1) {
    perror("getifaddrs");
    exit(EXIT_FAILURE);
  }
  
  /* Walk through linked list, maintaining head pointer so we
     can free list later. */
  
  for (struct ifaddrs *ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
    if (ifa->ifa_addr == NULL)
      continue;
    
    family = ifa->ifa_addr->sa_family;
    
    /* Display interface name and family (including symbolic
       form of the latter for the common families). */
    
    if (family != AF_PACKET) {
      if (tty) printf("%s", BOLD);
      printf("%-8s %s (%d)\n",
	     ifa->ifa_name,
	     (family == AF_PACKET) ? "AF_PACKET" :
	     (family == AF_INET) ? "AF_INET" :
	     (family == AF_INET6) ? "AF_INET6" : "???",
	     family);
      if (tty) printf("%s", END);
    }

    
    /* For an AF_INET* interface address, display the address. */
    
    if (family == AF_INET || family == AF_INET6) {
      s = getnameinfo(ifa->ifa_addr,
		      (family == AF_INET) ? sizeof(struct sockaddr_in) :
		      sizeof(struct sockaddr_in6),
		      host, NI_MAXHOST,
		      NULL, 0, NI_NUMERICHOST);
      if (s != 0) {
	printf("getnameinfo() failed: %s\n", gai_strerror(s));
	exit(EXIT_FAILURE);
      }
      
      printf("   HW address: ");
      cmd_printHWAddr(ifa->ifa_name);
      printf(", IP address: %s\n", host);
      printf("   Link: ");
      char ss[1000];
      sprintf(ss,"/sys/class/net/%s/carrier", ifa->ifa_name);
      dumpFile(ss);

      printf("   Speed: ");
      sprintf(ss,"/sys/class/net/%s/speed", ifa->ifa_name);
      dumpFile(ss);

      printf("   MTU: ");
      sprintf(ss,"/sys/class/net/%s/mtu", ifa->ifa_name);
      dumpFile(ss);

            printf("   Carrier changes: ");
      sprintf(ss,"/sys/class/net/%s/carrier_changes", ifa->ifa_name);
      dumpFile(ss);

      
    }
    /*else if (family == AF_PACKET && ifa->ifa_data != NULL) {
      struct rtnl_link_stats *stats = ifa->ifa_data;
      
      printf("\t\ttx_packets = %10u; rx_packets = %10u\n"
	     "\t\ttx_bytes   = %10u; rx_bytes   = %10u\n",
	     stats->tx_packets, stats->rx_packets,
	     stats->tx_bytes, stats->rx_bytes);
	     }*/
  }
  
  freeifaddrs(ifaddr);
}

void cmd_listAll() {
  printf("Commands: \n");
  for (size_t i = 0; i < sizeof(commands)/sizeof(commands[0]); i++) {
    printf("  %-10s \t %s\n", commands[i].name, commands[i].doc);
  }
}


void cmd_pwgen(int tty) {
  const size_t len = 32;
  double pwentropy = 0, bitsentropy = 0;
  unsigned char *pw = NULL;
  do {
    if (pw) {
      free(pw);
    }

    unsigned char *bits = randomGenerate(len);
    bitsentropy = entropyTotalBits(bits, len, 1);
    
    pw = passwordGenerate(bits, len);
    pwentropy = entropyTotalBits(pw, len, 1);
    free(bits);
    
  } while ((pwentropy < 200 || bitsentropy < 242) && keepRunning);

  char ss[1000];
  if (tty) printf("%s", BOLD);
  printf("generate random bits for length %zd: ", len);
  if (tty) printf("%s", END);

  sprintf(ss, "(%.1lf bits of entropy)", bitsentropy);
  colour_printString(ss, bitsentropy >= 242, "\n", tty);
  
  
  printf("%s ", pw);
  sprintf(ss, "(%.1lf bits of entropy)", pwentropy);
  colour_printString(ss, pwentropy >= 200, "\n", tty);
  
  free(pw);
}


size_t countDriveBlockDevices() {
  procDiskStatsType d;
  procDiskStatsInit(&d);
  procDiskStatsSample(&d);
  size_t count = 0;
  for (size_t i = 0; i < d.num; i++) {
    if ((d.devices[i].majorNumber == 8) || (d.devices[i].majorNumber == 254)) {
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
  printf("device   \tencrypt\t bits\t majmin\tGB\tvendor\t%-18s\n", "model");
  if (tty) printf("%s", END);

  for (size_t i = 0; i < d.num && keepRunning; i++) {
    if ((d.devices[i].majorNumber == 8) || (d.devices[i].majorNumber == 254)) {
      char path[1000];
      sprintf(path, "/dev/%s", d.devices[i].deviceName);
      size_t bdsize = blockDeviceSize(path);
      double entropy = 0;

      const int bufsize = 80*20*4096;
      unsigned char *buffer = memalign(4096, bufsize); assert(buffer);
      memset(buffer, 0, bufsize);

      int fd = open(path, O_RDONLY | O_DIRECT);
      if (fd) {
	int didread = read(fd, buffer, bufsize);
	if (didread) {
	  entropy = entropyTotalBits(buffer, bufsize, 1);
	  //	  fprintf(stderr,"%s read %d, entropy %lf\n", path, didread, entropy / bufsize);
	}
	close(fd);
      } else {
	perror(path);
      }
      free(buffer);
      int encrypted = 0;
      if (entropy/bufsize > 7.9) {
	encrypted = 1;
      }
      printf("/dev/%s\t%s\t %.1lf\t %zd:%zd\t%.0lf\t%s\t%-18s\n", d.devices[i].deviceName, encrypted?"Encrypt":"No", entropy/bufsize,d.devices[i].majorNumber, d.devices[i].minorNumber, TOGB(bdsize), d.devices[i].idVendor, d.devices[i].idModel);
    }
  }
  
  procDiskStatsFree(&d);
}
  


void cmd_status(const char *hostname, const int tty) {
  time_t t = time(NULL);
  struct tm tm = *localtime(&t);

  char timestring[1000];
  strftime(timestring, 999, "%c", &tm);
  if (tty) printf("%s", BOLD);
  printf("%s\n", timestring);
  if (tty) printf("%s", END);

  
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

  printf("\n");
}


int main() {
  syslogString("stush", "Start session");
  if (geteuid() != 0) {
    fprintf(stderr, "*error* app needs root. sudo chmod +s ...\n");
    syslogString("stush", "error. app needs root.");
    exit(1);
  }
  
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  setvbuf(stdout, NULL, _IONBF, 0);  // turn off buffering
  setvbuf(stderr, NULL, _IONBF, 0);  // turn off buffering
  

  char hostname[91], prefix[100];
  if (gethostname(hostname, 90)) {
    sprintf(hostname, "stush");
  }

  int tty = isatty(1);
  
  header(tty);
  cmd_status(hostname, tty);

  sprintf(prefix, "%s$ ", hostname);

  char *line = NULL;
  rl_bind_key ('\t', rl_insert);
  
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
    syslogString("stush", line); // log

    
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
	} else if (strcmp(commands[i].name, "entropy") == 0) {
	  cmd_calcEntropy(tty, line);
	} else if (strcmp(commands[i].name, "lsnic") == 0) {
	  cmd_listNICs(tty);
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

  syslogString("stush", "Close session");

  return 0;
}
