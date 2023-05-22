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

#include <math.h>


#include <netinet/in.h>
#include <arpa/inet.h>

#include <netdb.h>
#include <ifaddrs.h>
#include <linux/if_link.h>


#include "utils.h"
#include "jobType.h"
#include "devices.h"
#include "procDiskStats.h"

int keepRunning = 1;
int verbose = 0;
int TeReo = 0;
int German = 0;

void intHandler(int d) {
    if (d) {}
    //  fprintf(stderr,"got signal\n");
    keepRunning = 0;
}


int com_status();

typedef struct {
    char *name;            /* User printable name of the function. */
    char *doc;            /* Documentation for this function.  */
} COMMAND;

typedef struct {
  char *en;
  char *trans;
} translateType;

translateType trans_mi[] = {
			 {"Yes", "Āe"},
			 {"No", "Kāo"},
			 {"Not available", "Kāore e wātea"},
			 {"Available", "Wātea"},
			 {"secure sandpit", "wahi ripoata haumaru"},
			 {"Location", "Tūnga"},
			 {"Support", "Tautoko"},
			 {"Hardware Type", "Momo Pūkenga"},
			 {"Host", "Kaiwhakahaere"},
			 {"OS Type", "Pūnaha Whakahaere"},
			 {"Uptime (days)", "Wā whakahaere (ra)"},
			 {"Utilization", "Whakamahinga"},
			 {"Total RAM", "Mahara Katoa"},
			 {"Free RAM", "Mahara wātea"},
			 {"Swap space", "Wāhi Whakakapi"},
			 {"CPU Model", "Momo Tīhanga"},
			 {"CPU Cores", "Ngā Kēnga"},
			 {"SSH Passwords", "SSH Kupuhipa"},
			 {"Power saving mode", "Aratau Whakaiti Hiko"},
			 {"Block device count", "Tau Pūrere Pūtau"},
			 {"Count", "Tatau"},
			 {"Commands", "Whakahau"},
			 {"Show CPU info", "Whakaatu pūnaha whakahaere"},
			 {"Show the current date/time", "Whakaatu rā/tāima o nāianei"},
			 {"Disk free", "Pūtau wātea"},
			 {"Dropbear SSH config", "Whirihoranga SSH Dropbear"},
			 {"Calc entropy of a string", "Tātai entropi o te reta"},
			 {"Set locale language", "Whakatakoto reo"},
			 {"List drive block devices", "Whakaatu pūtau pūrere"},
			 {"List IP/HW addresses", "Whakaatu IP/Ngā wāhitau hangarau"},
			 {"Show mounts info", "Whakaatu pātengi raraunga"},
			 {"Generate cryptographically complex 200-bit random password", "Hanga kupuhipa matatini whakakotahitanga 200-bit"},
			 {"Measure read speed on device", "Ine tere pānui o te pūrere"},
			 {"Show SCSI devices", "Whakaatu pūrere SCSI"},
			 //			 {"Stu's powerful I/O tester", "
			 {"Show system status", "Whakaatu tūnga pūnaha"},
			 {"List translations", "Whakaatu whakamāoritanga"},
			 {"Is the terminal interactive", "He whakawhitiwhitiwhiti te whakawhitiwhiti"},
			 {"Exit the secure shell", "Whakakore i te kiriata haumaru"},
			 {"Usage", "Whakamahi"},
			 {"Examples", "Whakarāpopototanga"},
			 {"Hello", "Kia ora"},
			 {"Goodbye", "Kia pai tō rā"},
			 {"Type ? or help to list commands", "Pāwhiritia ? or āwhina hei whakaatu i ngā whakahau"},
			 {"Unknown command", "Whakahau kore e mōhio ana"}
};



translateType trans_de[] = {
    {"Yes", "Ja"},
    {"No", "Nein"},
    {"Not available", "Nicht verfügbar"},
    {"Available", "Verfügbar"},
    {"secure sandpit", "Sicherer Sandkasten"},
    {"Location", "Ort"},
    {"Support", "Unterstützung"},
    {"Hardware Type", "Hardwaretyp"},
    {"Host", "Host"},
    {"OS Type", "Betriebssystemtyp"},
    {"Uptime (days)", "Betriebszeit (Tage)"},
    {"Utilization", "Auslastung"},
    {"Total RAM", "Gesamter RAM"},
    {"Free RAM", "Freier RAM"},
    {"Swap space", "Auslagerungsdatei"},
    {"CPU Model", "CPU-Modell"},
    {"CPU Cores", "CPU-Kerne"},
    {"SSH Passwords", "SSH-Passwörter"},
    {"Power saving mode", "Energiesparmodus"},
    {"Block device count", "Anzahl der Blockgeräte"},
    {"Count", "Anzahl"},
    {"Commands", "Befehle"},
    {"Show CPU info", "CPU-Informationen anzeigen"},
    {"Show the current date/time", "Aktuelles Datum/Uhrzeit anzeigen"},
    {"Disk free", "Festplattenspeicher frei"},
    {"Dropbear SSH config", "Dropbear SSH-Konfiguration"},
    {"Calc entropy of a string", "Entropie eines Strings berechnen"},
    {"Set locale language", "Sprache festlegen"},
    {"List drive block devices", "Blockgeräte auflisten"},
    {"List IP/HW addresses", "IP-/HW-Adressen auflisten"},
    {"Show mounts info", "Mount-Informationen anzeigen"},
    {"Generate cryptographically complex 200-bit random password", "Kryptografisch komplexe Zufallspasswörter (200 Bit) generieren"},
    {"Measure read speed on device", "Lesegeschwindigkeit des Geräts messen"},
    {"Show SCSI devices", "SCSI-Geräte anzeigen"},
    {"Show system status", "Systemstatus anzeigen"},
    {"List translations", "Übersetzungen auflisten"},
    {"Is the terminal interactive", "Ist das Terminal interaktiv"},
    {"Exit the secure shell", "Sichere Shell beenden"},
    {"Usage", "Verwendung"},
    {"Examples", "Beispiele"},
    {"Hello", "Hallo"},
    {"Goodbye", "Auf Wiedersehen"},
    {"Type ? or help to list commands", "Geben Sie ? oder Hilfe ein, um Befehle aufzulisten"},
    {"Unknown command", "Unbekannter Befehl"}
};
			  

const char *T(const char *s) {
  for (size_t i = 0; i < sizeof(trans_mi)/sizeof(trans_mi[0]); i++) {
    if (strcasecmp(trans_mi[i].en, s)==0) {
      if (TeReo) return trans_mi[i].trans;
      if (German) return trans_de[i].trans;
      return s;
    }
  }
  return s;
}

const char *BOLD = "\033[1m";
const char *RED = "\033[31m";
const char *GREEN = "\033[32m";
const char *END = "\033[0m";


void cmd_translations(int tty) {
  if (tty) printf("%s", BOLD);
  printf("%-40s\t| %-40s\n", "en_NZ.UTF-8", TeReo?"mi_NZ.UTF-8":(German?"de_DE.UTF-8":"?"));
  if (tty) printf("%s", END);
  for (size_t i = 0; i < sizeof(trans_mi)/sizeof(trans_mi[0]); i++) {
    if (TeReo) {
      printf("%-40s\t| %-40s\n", trans_mi[i].en, trans_mi[i].trans);
    } else {
      printf("%-40s\t| %-40s\n", trans_de[i].en, trans_de[i].trans);
    }
  }
}

  


COMMAND commands[] = {
        {"cpu",       "Show CPU info"},
        {"date",      "Show the current date/time"},
        {"df",        "Disk free"},
        {"dropbear",  "Dropbear SSH config"},
        {"entropy",   "Calc entropy of a string"},
        {"lang",      "Set locale language"},
        {"lsblk",     "List drive block devices"},
        {"lsnic",     "List IP/HW addresses"},
        {"mounts",    "Show mounts info"},
        {"pwgen",     "Generate cryptographically complex 200-bit random password"},
        {"readspeed", "Measure read speed on device"},
        {"scsi",      "Show SCSI devices"},
        {"spit",      "Stu's powerful I/O tester"},
        {"status",    "Show system status"},
        {"translations",    "List translations"},
        {"tty",       "Is the terminal interactive"},
        {"exit",      "Exit the secure shell"}
};




void cmd_lang(const int tty, char *origstring) {
    char *string = strdup(origstring);
    const char *delim = " ";
    char *first = strtok(string, delim);
    if (first) {
        char *second = strtok(NULL, delim);
        if (second) {
            second = origstring + (second - string);
            if (setlocale(LC_ALL, second) == NULL) {
	      printf("LANG/locale '%s' is not available\n", second);
            } else {
                if (tty) printf("%s", BOLD);
                printf("LANG is %s\n", second);
                if (tty) printf("%s", END);

                TeReo = 0;
		German = 0;
                if (strncasecmp(second, "mi_NZ", 5) == 0) {
                    TeReo = 1;
		}
                if (strncasecmp(second, "de_DE", 5) == 0) {
		    German = 1;
		}
		if (tty) printf("%s", BOLD);
		printf("%s\n", T("Hello"));
		if (tty) printf("%s", END);
	    }
        } else {
	  printf("%s: %s <locale>   (e.g. en_NZ.UTF-8, mi_NZ.UTF-8)\n", T("Usage"), first);
        }
    }
    free(string);
}


void header(const int tty) {
    char *lang = getenv("LANG");
    if (lang == NULL) {
        lang = strdup("en_US.utf8");
    } else {
        lang = strdup(lang);
    }

    char ss[PATH_MAX];
    sprintf(ss, "lang %s", lang);
    cmd_lang(tty, ss);

    if (tty) {
        printf("%s", BOLD);
    }
    printf("stush: (%s: v0.2)\n\n", T("secure sandpit"));
    if (tty) {
        printf("%s", END);
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

    if (string) printf("%s", string);

    printf("%s", suffix);
    if (tty) {
        printf("%s", END);
    }
}

void usage_spit() {
    printf("%s: spit <device> <command> ... <command>\n", T("Usage"));
    printf("\n%s: \n", T("Examples"));
    printf("  spit <device> rs0           --- random 4KiB read \n");
    printf("  spit <device> rzs1k64       --- seq 64KiB read\n");
    printf("  spit <device> rs1k64 ws1k4  --- a 64KiB read thread and a 4KiB write thread\n");
}


void cmd_spit(const int tty, char *origstring) {
    char *string = strdup(origstring);
    const char *delim = " ";
    char *first = strtok(string, delim);
    if (first) {
        char *second = strtok(NULL, delim);

        if (second == NULL) {
            usage_spit();
        } else {
            char *third = NULL;

            jobType j;
            jobInit(&j);

            while ((third = strtok(NULL, delim))) {
                jobAdd(&j, third);
            }

            if (j.count == 0) {
                usage_spit();
            } else {
                size_t bdsize = blockDeviceSize(second);
                if (bdsize == 0) {
                    perror(second);
                } else {
                    if (canOpenExclusively(second) == 0) { //0 is no
                        perror(second);
                    } else {
                        jobAddDeviceToAll(&j, second);
                        if (tty) printf("%s", BOLD);
                        jobDump(&j);
                        if (tty) printf("%s", END);

                        jobRunThreads(&j, j.count, NULL, 0, bdsize, 30, 0, NULL, 4, 42, 0, NULL, 1, 0, 0, NULL, NULL,
                                      NULL, "all", 0, /*&r*/NULL, 15L * 1024 * 1024 * 1024, 0, 0, NULL, 0, 1);
                        jobFree(&j);
                    }
                }
            }
        }
    }
    free(string);
}


void cmd_calcEntropy(const int tty, char *origstring) {
    char *string = strdup(origstring);
    const char *delim = " ";
    char *first = strtok(string, delim);
    if (first) {
        char *second = strtok(NULL, delim);
        if (second) {
            second = origstring + (second - string);
            double entropy = entropyTotalBits((unsigned char *) second, strlen(second), 1);
            printf("%s ", second);
            char ss[PATH_MAX];
            sprintf(ss, "(%.1lf bits of entropy)", entropy);
            colour_printString(ss, entropy >= 200, "\n", tty);
        } else {
            printf("usage: %s <string>\n", first);
        }
    }
    free(string);
}


void cmd_tty(int tty) {
    printf("tty: %s\n", tty ? T("Yes") : T("No"));
}

void cmd_env(int tty) {
    if (tty) {}

    char **env = environ;
    for (; *env; ++env) {
        printf("%s\n", *env);
    }
}


void cmd_printHWAddr(char *nic) {

    int s;
    struct ifreq buffer;

    s = socket(PF_INET, SOCK_DGRAM, 0);
    memset(&buffer, 0x00, sizeof(buffer));
    strcpy(buffer.ifr_name, nic);
    ioctl(s, SIOCGIFHWADDR, &buffer);
    close(s);

    for (s = 0; s < 6; s++) {
        if (s > 0) printf(":");
        printf("%.2x", (unsigned char) buffer.ifr_hwaddr.sa_data[s]);
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
            char ss[PATH_MAX];
            sprintf(ss, "/sys/class/net/%s/carrier", ifa->ifa_name);
            dumpFile(ss, "", 0);

            printf("   Speed: ");
            sprintf(ss, "/sys/class/net/%s/speed", ifa->ifa_name);
            dumpFile(ss, "", 0);

            printf("   MTU: ");
            sprintf(ss, "/sys/class/net/%s/mtu", ifa->ifa_name);
            dumpFile(ss, "", 0);

            printf("   Carrier changes: ");
            sprintf(ss, "/sys/class/net/%s/carrier_changes", ifa->ifa_name);
            dumpFile(ss, "", 0);


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
    printf("%s: \n", T("Commands"));
    for (size_t i = 0; i < sizeof(commands) / sizeof(commands[0]); i++) {
       printf("  %-10s \t| %s\n", commands[i].name, T(commands[i].doc));
    }
}

// 16 is the minimum
void cmd_pwgen(int tty, char *origstring) {
    size_t len = 16, targetbits = 200;

    char *string = strdup(origstring);
    const char *delim = " ";
    char *first = strtok(string, delim);
    if (first) {
        char *second = strtok(NULL, delim);
        if (second) {
            targetbits = atoi(second);
        }
    }
    free(string);

    double pwentropy = 0, bitsentropy = 0;
    unsigned char *pw = NULL;

    // try 1,000 times
    int found = 0;
    for (size_t i = 0; i < 1000 && keepRunning; i++) {
        if (pw) {
            free(pw);
        }

        unsigned char *bits = randomGenerate(len);
        bitsentropy = entropyTotalBytes(bits, len);

        pw = passwordGenerate(bits, len);
        pwentropy = entropyTotalBytes(pw, len);
        free(bits);

        if (pwentropy > targetbits && bitsentropy > targetbits) {
            found = 1;
            break;
        } else {
            //      printf("*warning* target of length %zd, achieved only %lf bits\n", len, pwentropy);
            len++;
        }
    }

    char ss[PATH_MAX];
    if (tty) printf("%s", BOLD);
    printf("generate random bits for length %zd: ", len);
    if (tty) printf("%s", END);

    sprintf(ss, "(%.1lf bits of entropy)", bitsentropy);
    colour_printString(ss, bitsentropy >= 200, "\n", tty);


    printf("%s ", pw);
    sprintf(ss, "(%.1lf bits of entropy, %.2lf bpc)", pwentropy, pwentropy / len);
    colour_printString(ss, pwentropy >= 200, "\n", tty);
    if (found == 0) {
        if (tty) printf("%s", BOLD);
        printf("*warning: password is weak and is below entropy target\n");
        if (tty) printf("%s", END);
    }


    free(pw);
}


size_t countDriveBlockDevices() {
    procDiskStatsType d;
    procDiskStatsInit(&d);
    procDiskStatsSample(&d);
    size_t count = 0;

    size_t majCount[1024];
    memset(&majCount, 0, sizeof(size_t) * 1024);

    for (size_t i = 0; i < d.num; i++) {
        size_t mn = d.devices[i].majorNumber;
        if (mn > 1024) mn = 1023;
        majCount[mn]++;

        if ((d.devices[i].majorNumber == 8) || (d.devices[i].majorNumber == 254)) {
            int len = strlen(d.devices[i].deviceName);
            char lastchar = d.devices[i].deviceName[len - 1];
            //      printf("%s %c\n", d.devices[i].deviceName, lastchar);
            if (!isdigit(lastchar)) {
                count++;
                // check serial is unique
            }
        }
    }
    printf("\n");
    for (size_t i = 0; i < 1024; i++) {
        if (majCount[i]) {
            char *majString = majorBDToString(i);
            printf("   %s[%-6s]  =  %3zd (major=%zd)\n", T("Count"), majString, majCount[i], i);
            free(majString);
        }
    }
    return count;
}


void cmd_listDriveBlockDevices(int tty, char *origstring) {
    if (tty) {}
    size_t major = 0;

    char *string = strdup(origstring);
    const char *delim = " ";
    char *first = strtok(string, delim);
    if (first) {
        char *second = strtok(NULL, delim);
        if (second) {
            major = atoi(second);
        }
    }
    free(string);


    procDiskStatsType d;
    procDiskStatsInit(&d);
    procDiskStatsSample(&d);

    if (tty) printf("%s", BOLD);
    printf("device   \tencrypt\t bits\t majmin\tGB\tvendor\t%-18s\t%-10s\n", "model", "serial");
    if (tty) printf("%s", END);

    for (size_t i = 0; i < d.num && keepRunning; i++) {
        if ((major == 0) || (d.devices[i].majorNumber == major)) {
            char path[PATH_MAX];
            sprintf(path, "/dev/%s", d.devices[i].deviceName);
            size_t bdsize = blockDeviceSize(path);
            double entropy = NAN;

            const int bufsize = 80 * 20 * 4096;
            unsigned char *buffer = memalign(4096, bufsize);
            assert(buffer);
            memset(buffer, 0, bufsize);

            int fd = open(path, O_RDONLY | O_DIRECT);
            char *serial = NULL;
            if (fd) {
                int didread = read(fd, buffer, bufsize);
                if (didread) {
                    entropy = entropyTotalBytes(buffer, bufsize);
                    serial = serialFromFD(fd);
                    //	  	  fprintf(stderr,"%s read %d, entropy %lf\n", path, didread, entropy / bufsize);
                }
                close(fd);
            } else {
                perror(path);
            }
            free(buffer);
            int encrypted = 0;
            if (entropy / bufsize > 7.9) {
                encrypted = 1;
            }
            printf("/dev/%s\t%s\t %.1lf\t %zd:%zd\t%.0lf\t%s\t%-18s\t%-10s\n", d.devices[i].deviceName,
                   encrypted ? "Encrypt" : "No", entropy / bufsize, d.devices[i].majorNumber, d.devices[i].minorNumber,
                   TOGB(bdsize), d.devices[i].idVendor, d.devices[i].idModel, serial ? serial : "can't open");
            free(serial);
        }
    }

    procDiskStatsFree(&d);
}

void cmd_cpu(const int tty) {
    if (tty) {}
    dumpFile("/proc/cpuinfo", "(vendor|name|mhz|cores|stepping|cache|bogo)", 0);
}

void cmd_mounts(const int tty) {
    if (tty) {}
    dumpFile("/proc/mounts", "^/", 0);
}

void cmd_dropbear(const int tty) {
    if (tty) {}
    dumpFile("/etc/initramfs-tools/conf.d/dropbear", "^IP=", 0);
}

void cmd_df(const int tty, char *origstring) {
    if (tty) {}
    char *string = strdup(origstring);
    const char *delim = " ";
    char *first = strtok(string, delim);
    if (first) {
        char *second = strtok(NULL, delim);
        if (second) {
            diskSpaceFromMount(second);
        } else {
            printf("usage: df <mountpoint>\n");
        }
    }
    free(string);
}

void cmd_scsi(const int tty) {
    if (tty) {}
    dumpFile("/proc/scsi/scsi", "", 0);
}

void cmd_date(const int tty) {
    time_t t = time(NULL);
    struct tm tm = *localtime(&t);

    char timestring[PATH_MAX];
    strftime(timestring, 999, "%c", &tm);
    if (tty) printf("%s", BOLD);
    printf("%s\n", timestring);
    if (tty) printf("%s", END);
}


void cmd_status(const char *hostname, const int tty) {

    cmd_date(tty);

    printf("%-20s\t", T("Location"));
    colour_printString(getenv("LOCATION"), 1, "\n", tty);

    printf("%-20s\t", T("Support"));
    colour_printString(getenv("SUPPORT"), 1, "\n", tty);

    printf("%-20s\t", T("Hardware Type"));
    colour_printString(getenv("HARDWARE_TYPE"), 1, "\n", tty);

    char *os = OSRelease();
    printf("%-20s\t", T("Host"));
    colour_printString(hostname, 1, "\n", tty);


    printf("%-20s\t", T("OS Type"));
    colour_printString(os, 1, "\n", tty);
    if (os) free(os);

    printf("%-20s\t", T("Uptime (days)"));
    printf("%.0lf\n", getUptime() / 86400.0);

    printf("%-20s\t", T("Utilization")); // load average
    printf("%.1lf\n", loadAverage());
    
    printf("%-20s\t", T("Total RAM"));
    printf("%.0lf GiB\n", TOGiB(totalRAM()));

    printf("%-20s\t", T("Free RAM"));
    colour_printNumber(TOGiB(freeRAM()), TOGiB(freeRAM()) >= 1, " GiB\n", tty);

    printf("%-20s\t", T("Swap space"));
    printf("%.0lf GB\n", TOGB(swapTotal()));

    char *cpu = getCPUModel();
    printf("%-20s\t", T("CPU Model"));
    printf("%s\n", cpu);
    if (cpu) free(cpu);

    printf("%-20s\t", T("CPU Cores"));
    printf("%d\n", cpuCountPerNuma(0) * getNumaCount());


    printf("%-20s\t", "Dropbear");
    int dropbear = 0;
    FILE *fp = fopen("/etc/initramfs-tools/conf.d/dropbear", "rt");
    if (fp) {
        dropbear = 1;
        fclose(fp);
    }
    colour_printString(dropbear ? T("Yes") : T("No"), dropbear, "\n", tty);

    printf("%-20s\t", T("SSH Passwords"));
    int sshpasswords = dumpFile("/etc/ssh/sshd_config", "^PasswordAuthentication no", 1);
    colour_printString((sshpasswords == 1) ? T("No") : T("Yes"), sshpasswords == 1, "\n", tty);


    char *power = getPowerMode();
    printf("%-20s\t", T("Power saving mode"));
    colour_printString(power, (strcasecmp("performance", power) == 0), "\n", tty);
    if (power) free(power);

    /*  printf("%-20s\t", "Entropy Available");
    int entropy = entropyAvailable();
    colour_printNumber(entropy, entropy > 200, " bits\n", tty);*/

    printf("%-20s\t", T("Block device count"));
    countDriveBlockDevices();
    //  colour_printNumber(drives, drives > 0, " \n", tty);

    printf("\n");
}


void help_prompt() {
  printf("%s\n", T("Type ? or help to list commands"));
}


void run_command(int tty, char *line, char *hostname) {
    int known = 0;
    for (size_t i = 0; i < sizeof(commands) / sizeof(COMMAND); i++) {
        if (commands[i].name && (strncmp(line, commands[i].name, strlen(commands[i].name)) == 0)) {
            if (strcasecmp(commands[i].name, "status") == 0) {
                cmd_status(hostname, tty);
	    } else if (strcasecmp(commands[i].name, "translations") == 0) {
	      cmd_translations(tty);
            } else if (strcasecmp(commands[i].name, "pwgen") == 0) {
                cmd_pwgen(tty, line);
            } else if (strcasecmp(commands[i].name, "lsblk") == 0) {
                cmd_listDriveBlockDevices(tty, line);
            } else if (strcasecmp(commands[i].name, "entropy") == 0) {
                cmd_calcEntropy(tty, line);
            } else if (strcasecmp(commands[i].name, "cpu") == 0) {
                cmd_cpu(tty);
            } else if (strcasecmp(commands[i].name, "dropbear") == 0) {
                cmd_dropbear(tty);
            } else if (strcasecmp(commands[i].name, "mounts") == 0) {
                cmd_mounts(tty);
            } else if (strcasecmp(commands[i].name, "scsi") == 0) {
                cmd_scsi(tty);
            } else if (strcasecmp(commands[i].name, "spit") == 0) {
                cmd_spit(tty, line);
            } else if (strcasecmp(commands[i].name, "df") == 0) {
                cmd_df(tty, line);
            } else if (strcasecmp(commands[i].name, "date") == 0) {
                cmd_date(tty);
            } else if (strcasecmp(commands[i].name, "lang") == 0) {
                cmd_lang(tty, line);
		help_prompt();
            } else if (strcasecmp(commands[i].name, "tty") == 0) {
                cmd_tty(tty);
                //	} else if (strcasecmp(commands[i].name, "env") == 0) {
                //	  cmd_env(tty);
            } else if (strcasecmp(commands[i].name, "lsnic") == 0) {
                cmd_listNICs(tty);
            } else if (strcasecmp(commands[i].name, "readspeed") == 0) {
                char *second = strchr(line, ' ');
                if (second) {
                    int len = strlen(second + 1);
                    if (len > 0) {
                        second++;

                        int fd = open(second, O_RDONLY | O_DIRECT);
                        if (fd < 0) {
                            perror(second);
                        } else {
                            unsigned int major, minor;
                            if (majorAndMinor(fd, &major, &minor) != 0) {
                                printf("*warning* can't get major:minor for '%s'\n", second);
                            }
                            if (tty) printf("%s", BOLD);
                            fprintf(stdout, "*info* readspeed '%s', size=2 MiB for 5 seconds (MB/s)\n", second);
                            if (tty) printf("%s", END);
                            readSpeed(fd, 5, 2L * 1024 * 1024);
                            close(fd);
                        }
                    }
                } else {
                    printf("usage: readspeed <device>\n");
                }

            }
            known = 1;
            break;
        }
    }
    if ((strcasecmp(line, "?") == 0) || (strcasecmp(line, "help") == 0)) {
        cmd_listAll();
    } else {
        if (!known) {
  	    printf("%s: %s\n", line, T("Unknown command"));
        }
    }
}


int main(int argc, char *argv[]) {

    syslogString("stush", "Start session");
    if (geteuid() != 0) {
        fprintf(stderr, "*error* app needs root. sudo chmod +s ...\n");
        syslogString("stush", "error. app needs root.");
        exit(1);
    }

    loadEnvVars("/etc/stush.cfg");

    signal(SIGTERM, intHandler);
    signal(SIGINT, intHandler);
    setvbuf(stdout, NULL, _IONBF, 0);  // turn off buffering
    setvbuf(stderr, NULL, _IONBF, 0);  // turn off buffering


    char hostname[NAME_MAX - 10], prefix[NAME_MAX];
    if (gethostname(hostname, NAME_MAX - 11)) {
        sprintf(hostname, "stush");
    }

    int tty = isatty(1);


    // cli
    for (int i = 1; i < argc; i++) {
        if (argv[i][0] != '-') {
            run_command(tty, argv[i], hostname);
            exit(0);
        }
    }


    header(tty);
    cmd_status(hostname, tty);

    help_prompt();
    sprintf(prefix, "%s$ ", hostname);

    char *line = NULL;
    rl_bind_key('\t', rl_insert);

    while (1) {
        keepRunning = 1;
        line = readline(prefix);

        if ((line == NULL) || (strcasecmp(line, "exit") == 0) || (strcasecmp(line, "quit") == 0)) {
            break;
        }
        if (strlen(line) < 1) {
            free(line);
            continue;
        }
        syslogString("stush", line); // log


        add_history(line);

        run_command(tty, line, hostname);
    }

    if (line == NULL) printf("\n");

    if (TeReo) {
        if (tty) printf("%s", BOLD);
        printf("%s\n", T("Goodbye"));
        if (tty) printf("%s", END);
    }

    free(line);

    syslogString("stush", "Close session");

    return 0;
}
