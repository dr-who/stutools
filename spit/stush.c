#define _GNU_SOURCE

#include <stdio.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>

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
#include <limits.h>

#include <netinet/in.h>
#include <arpa/inet.h>

#include <netdb.h>
#include <ifaddrs.h>
#include <linux/if_link.h>

#include <sys/timex.h>


#include "utils.h"
#include "jobType.h"
#include "devices.h"
#include "procDiskStats.h"
#include "numList.h"

#include "snack.h"
#include "dns.h"

#include "pciUtils.h"

int keepRunning = 1;
int verbose = 0;
int TeReo = 0;
int German = 0;

void intHandler(int d) {
    if (d==SIGALRM) {
      syslogString("stush", "Timeout alarm");
      syslogString("stush", "Close session");
      fprintf(stderr,"Inactivity timeout\n");
      exit(1);
    }
    //  fprintf(stderr,"got signal\n");
    keepRunning = 0;
}


void funHeader() {
  printf("     _             _     \n");
  printf(" ___| |_ _   _ ___| |__  \n");
  printf("/ __| __| | | / __| '_ \\ \n");
  printf("\\__ \\ |_| |_| \\__ \\ | | |\n");
  printf("|___/\\__|\\__,_|___/_| |_|\n");
}

  

int com_status();

typedef struct {
    char *name;            /* User printable name of the function. */
    char *doc;            /* Documentation for this function.  */
    char *admin;          /* admin user required */
} COMMAND;

typedef struct {
  char *en;
  char *trans;
  char *source;
} translateType;

#include "translate_en_mi.c"

translateType trans_de[] = {
    {"Yes", "Ja", ""},
    {"No", "Nein", ""},
    {"Not available", "Nicht verfügbar", ""},
    {"Available", "Verfügbar", ""},
    {"secure sandpit", "Sicherer Sandkasten", ""},
    {"Location", "Ort", ""},
    {"Support", "Unterstützung", ""},
    {"Hardware Type", "Hardwaretyp", ""},
    {"Host", "Host", ""},
    {"OS Type", "Betriebssystemtyp", ""},
    {"Uptime (days)", "Betriebszeit (Tage)", ""},
    {"Utilization", "Auslastung", ""},
    {"Total RAM", "Gesamter RAM", ""},
    {"Free RAM", "Freier RAM", ""},
    {"Swap space", "Auslagerungsdatei", ""},
    {"CPU Model", "CPU-Modell", ""},
    {"CPU Cores", "CPU-Kerne", ""},
    {"SSH Passwords", "SSH-Passwörter", ""},
    {"Power saving mode", "Energiesparmodus", ""},
    {"Block device count", "Anzahl der Blockgeräte", ""},
    {"Count", "Anzahl", ""},
    {"Commands", "Befehle", ""},
    {"Show CPU info", "CPU-Informationen anzeigen", ""},
    {"Show the current date/time", "Aktuelles Datum/Uhrzeit anzeigen", ""},
    {"Disk free", "Festplattenspeicher frei", ""},
    {"Dropbear SSH config", "Dropbear SSH-Konfiguration", ""},
    {"Calc entropy of a string", "Entropie eines Strings berechnen", ""},
    {"Set locale language", "Sprache festlegen", ""},
    {"List drive block devices", "Blockgeräte auflisten", ""},
    {"List IP/HW addresses", "IP-/HW-Adressen auflisten", ""},
    {"Show mounts info", "Mount-Informationen anzeigen", ""},
    {"Generate cryptographically complex 200-bit random password", "Kryptografisch komplexe Zufallspasswörter (200 Bit) generieren", ""},
    {"Measure read speed on device", "Lesegeschwindigkeit des Geräts messen", ""},
    {"Show SCSI devices", "SCSI-Geräte anzeigen", ""},
    {"Show system status", "Systemstatus anzeigen", ""},
    {"List translations", "Übersetzungen auflisten", ""},
    {"Is the terminal interactive", "Ist das Terminal interaktiv", ""},
    {"Exit the secure shell", "Sichere Shell beenden", ""},
    {"Usage", "Verwendung", ""},
    {"Examples", "Beispiele", ""},
    {"Hello", "Hallo", ""},
    {"Goodbye", "Auf Wiedersehen", ""},
    {"Type ? or help to list commands", "Geben Sie ? oder Hilfe ein, um Befehle aufzulisten", ""},
    {"Unknown command", "Unbekannter Befehl", ""}
};
			  

const char *T(const char *s) {
  if (TeReo) {
    for (size_t i = 0; i < sizeof(trans_mi)/sizeof(trans_mi[0]); i++) {
      if (strcasecmp(trans_mi[i].en, s)==0) {
	return trans_mi[i].trans;
      }
    }
  } else if (German) {
    for (size_t i = 0; i < sizeof(trans_de)/sizeof(trans_de[0]); i++) {
      if (strcasecmp(trans_de[i].en, s)==0) {
	return trans_de[i].trans;
      }
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
  printf("%-40s\t| %-60s\n", "en_NZ.UTF-8", TeReo?"mi_NZ.UTF-8":(German?"de_DE.UTF-8":"?"));
  if (tty) printf("%s", END);
  FILE *fp = fopen("translations.txt", "wt");
  if (TeReo) {
    for (size_t i = 0; i < sizeof(trans_mi)/sizeof(trans_mi[0]); i++) {
      printf("%-40s\t| %-40s\n", trans_mi[i].en, trans_mi[i].trans);
      if (fp) {
	fprintf(fp, "%s\t%s\t%s\n", trans_mi[i].en, trans_mi[i].trans, trans_mi[i].source);
      }
    }
  } else if (German) {
    for (size_t i = 0; i < sizeof(trans_de)/sizeof(trans_de[0]); i++) {
      printf("%-40s\t| %-40s\n", trans_de[i].en, trans_de[i].trans);
      if (fp) {
	fprintf(fp, "%s\t%s\t%s\n", trans_de[i].en, trans_de[i].trans, trans_de[i].source);
      }
    }
  }
  if (fp) {
    fclose(fp);
  }
}

  


COMMAND commands[] = {
        {"admin",     "Enter admin mode", ""},
        {"ascii",     "Show ASCII info for a character", ""},
	{"authenv",   "Reset TOTP key from ENV", "admin"},
	{"authgen",   "Generate TOTP key", "admin"},
	{"authls",    "Show the TOTP key", "admin"},
	{"authclear", "Clear the TOTP key", "admin"},
	{"authqr",    "Show the TOTP QR code", "admin"},
	{"authset",   "Set the TOTP key", "admin"},
	{"authtok",   "Show the current TOTP token", "admin"},
        {"cpu",       "Show CPU info", "admin"},
        {"date",      "Show the current date/time", ""},
	{"d2b",       "Decimal to binary", ""},
	{"d2h",       "Decimal to hex", ""},
        {"devlatency", "Measure read latency on device", "admin"},
        {"devspeed",  "Measure read speed on device", "admin"},
        {"df",        "Disk free", "admin"},
        {"dnsadd",    "Add DNS server", "admin"},
        {"dnsclear",  "Remove all DNS servers", "admin"},
        {"dnsrm",     "Remove a specific DNS server", "admin"},
        {"dnsls",     "List DNS servers", "admin"},
        {"dnsrc",     "Load DNS servers from /etc/resolv.conf", "admin"},
        {"dnstest",   "Measure DNS latencies", "admin"},
        {"dropbear",  "Dropbear SSH config", "admin"},
        {"entropy",   "Calc entropy of a string", ""},
        {"env",       "List environment variables", "admin"},
        {"h2d",        "Hex to decimal", ""},
        {"host",      "Convert hostname to IP", ""},
        {"id",        "Shows process IDs", "admin"},
        {"ip",        "List IPv4 information", ""},
        {"lang",      "Set locale language", ""},
        {"last",      "Show previous users", "admin"},
        {"lsblk",     "List drive block devices", "admin"},
        {"lsnic",     "List IP/HW addresses", "admin"},
        {"lspci",     "List PCI devices", "admin"},
        {"mem",      "Show memory usage", "admin"},
        {"mounts",    "Show mounts info", "admin"},
        {"netserver", "Starts server for network tests", ""},
        {"netclient", "Client connects to a server for tests", ""},
        {"ntp",       "Show NTP/network time status", ""},
        {"raidsim",   "Simulate k+m parity/RAID durability", ""},
        {"ps",        "Lists the number of processes", "admin"},
        {"top",       "Lists the running processes", "admin"},
        {"pwgen",     "Generate cryptographically complex 200-bit random password", ""},
	{"qr",        "Generate QR code", ""},
	{"route",     "Show route info", "admin"},
        {"scsi",      "Show SCSI devices", "admin"},
        {"hexascii",  "Display ENV variable in ASCII format", "admin"},
        {"hexdump",   "Dump ENV variable in spaced hex format", "admin"},
        {"hexset",    "Set ENV variable in hex format", "admin"},
        {"hexstring", "Convert string to hex ENV", "admin"},
	{"salt",      "Prefix an ENV variable with salt", "admin"},
	{"setenv",    "Set ENV", "admin"},
        {"sleep",     "Sleep for a few seconds", ""},
        {"spit",      "Stu's powerful I/O tester", "admin"},
        {"status",    "Show system status", ""},
        {"swap",      "Show swap status", "admin"},
        {"timeaudit", "Accept external time", ""},
        {"timeout",   "Show the auto logout timeout", ""},
        {"time",      "Show the time in seconds", ""},
        {"translations",    "List translations", "admin"},
        {"tty",       "Is the terminal interactive", ""},
        {"uptime",    "Time since the system booted", ""},
        {"who",       "List active users", ""},
        {"wifiqr",    "Create WIFI QR login", ""},
        {"exit",      "Exit the secure shell", ""}
};




void cmd_lang(const int tty, const char *second, const int quiet) {
  if (second == NULL) {
    printf("%s: %s <locale>   (e.g. en_NZ.UTF-8, mi_NZ.UTF-8)\n", T("Usage"), "lang");
    return; // as it's NULL
  } else if (setlocale(LC_ALL, second) == NULL) {
    printf("LANG/locale '%s' is not available\n", second);
  } else {
    if (quiet == 0)  {
      if (tty) printf("%s", BOLD);
      printf("LANG is %s\n", second);
      if (tty) printf("%s", END);
    }
  }
  
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


void header(const int tty) {
    char *lang = getenv("LANG");
    if (lang == NULL) {
        lang = strdup("en_US.utf8");
    } else {
        lang = strdup(lang);
    }

    char ss[PATH_MAX];
    sprintf(ss, "%s", lang);
    cmd_lang(tty, ss, 1);

    if (tty) {
        printf("%s", BOLD);
    }
    printf("stush: (%s: v0.85)\n\n", T("secure sandpit"));
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



void cmd_route() {
  dumpFile("/proc/net/route", "", 0);
}

#include "qr/qrcodegen.h"

static void printQr(const uint8_t qrcode[]) {
	int size = qrcodegen_getSize(qrcode);
	int border = 4;
	for (int y = -border; y < size + border; y++) {
		for (int x = -border; x < size + border; x++) {
			fputs((qrcodegen_getModule(qrcode, x, y) ? "\x1b[1;40m  \x1b[0;m" : "\x1b[1;107m  \x1b[0;m"), stdout);
		}
		//		fputs("\x1b[0;m", stdout);
		fputs("\n", stdout);
	}
}

void cmd_qr(int tty, const char *text) {
  if (text) {
    if (tty) printf("%s", BOLD);
    printf("qrcode: qr %s\n", text);
    if (tty) printf("%s", END);
    
    //	const char *text = "https://example.com";                // User-supplied text
	enum qrcodegen_Ecc errCorLvl = qrcodegen_Ecc_LOW;  // Error correction level
	
	// Make and print the QR Code symbol
	uint8_t qrcode[qrcodegen_BUFFER_LEN_MAX];
	uint8_t tempBuffer[qrcodegen_BUFFER_LEN_MAX];
	bool ok = qrcodegen_encodeText(text, tempBuffer, qrcode, errCorLvl,
		qrcodegen_VERSION_MIN, qrcodegen_VERSION_MAX, qrcodegen_Mask_AUTO, true);
	if (ok)
		printQr(qrcode);
  } else {
    printf("usage: qr <encoded URI/text>\n");
  }
}


void cmd_hexdump(const int tty, const char *rest) {
  if (tty) {}
  int showUsage = 1;
  if (rest) {
    char *s = getenv(rest);
    if (s) {
      size_t hexlen;
      unsigned char *hex = hexString(s, &hexlen);
      for (size_t i = 0;i < hexlen; i++) {
	printf("%02x", hex[i]);
	printf(" ");
      }
      printf("\n");
      showUsage = 0;
    }
  }
  if (showUsage) printf("usage: hexdump <variable>\n");
}


void cmd_setenv(const int tty, const char *rest) {
  if (tty) {}
  int showUsage = 1;
  if (rest) {
    char *copy = strdup(rest);
    char *first = strtok(copy, " ");
    if (first) {
      char *second = strtok(NULL, " ");
      if (second) {
	  
	const char *val = rest + (second - copy);

	setenv(first, val, 1);
	showUsage = 0;
      }
    }
  }
  if (showUsage) printf("usage: setenv <variable> <string...>\n");
}

void cmd_encodeASCII(const int tty, const char *rest) {
  if (tty) {}
  int showUsage = 1;
  if (rest) {
    char *copy = strdup(rest);
    char *first = strtok(copy, " ");
    if (first) {
      char *s = getenv(first);
      if (s) {
	size_t hexlen;
	unsigned char *hex = hexString(s, &hexlen);
	for (size_t i = 0;i < hexlen; i++) {
	  printf("%c", hex[i]);
	}
	printf(" (%zd bytes, %zd bits)\n", hexlen, hexlen * 8);
	showUsage = 0;
      }
    }
    free(copy);
  }
  if (showUsage) printf("usage: hexascii <variable>\n");
}

// hexstring S hello world
void cmd_hexstring(const int tty, const char *rest) {
  if (tty) {}

  int showUsage = 1;
  
  if (rest) {
    char *copy = strdup(rest);
    char *first = strtok(copy, " ");
    if (first) {
      char *second = strtok(NULL, " ");
      if (second) {
	  
	const char *val = rest + (second - copy);
	size_t len = strlen(val);
	char *store = calloc(len * 2 + 1, 1);
	const char *origstore = store;
	for (size_t i = 0; i < len; i++) {
	  store += sprintf(store, "%02x", val[i]);
	}
	*store = 0;
	setenv(first, strdup(origstore), 0);
	if (strcmp(getenv(first), origstore)) {
	  printf("error: couldn't change '%s'\n", first);
	} else {
	  printf("set '%s' = '%s'\n", first, origstore);
	}
	  
	showUsage = 0;
      }
    }
    free(copy);
  }
  if (showUsage) printf("usage: hexstring <variable> <string...>\n");
}
	  
  
// set c 1
void cmd_hexset(const int tty, const char *rest) {
  if (tty) {}
  
  if (rest) {
    char *copy = strdup(rest);
    char *first = strtok(copy, " ");
    if (first) {
      char *second = strtok(NULL, " ");
      if (second) {
	  
	const char *val = rest + (second - copy);
	
	size_t hexlen;
	unsigned char *hex = hexString(val, &hexlen);
	if (hexlen) { // then a hex string
	  char *s = calloc(hexlen * 2 + 1, 1);
	  const char *starts = s;
	  //	  s += sprintf(s, "x"); // 16# or x#
	  for (size_t i =0; i <hexlen; i++) {
	    s += sprintf(s, "%02x", hex[i]); 
	  }
	  *s = 0;
	  printf("%s (%zd bits)\n", starts, hexlen * 8);
	  // normal string
	  setenv(first, strdup(starts), 0);
	  if (strcmp(getenv(first), starts)) {
	    printf("error: couldn't change '%s'\n", first);
	  }
	}
      }
    }
  }
}

// salt variable string
void cmd_addSalt(const int tty, const char *rest) {
  if (tty) {}

  int showUsage = 1;
  if (rest) {
    char *copy = strdup(rest);
    char *first = strtok(copy, " ");
    if (first) {
      char *second = strtok(NULL, " ");
      if (second) {
	  
	const char *val = rest + (second - copy);


	size_t len = strlen(val);
	char *store = calloc(len * 2 + 1, 1);
	char *origstore = store;
	for (size_t i = 0; i < len; i++) {
	  store += sprintf(store, "%02x", val[i]);
	}
	*store = 0;

	printf("new string: %s\n", origstore);
	
	printf("old string: %s\n", getenv(first));

	char *concat = calloc(strlen(origstore) + strlen(getenv(first)) + 1, 1);
	strcat(concat, origstore);
	strcat(concat, getenv(first));
	
	// normal string
	setenv(first, concat, 1);
	if (strcmp(getenv(first), concat)) {
	  printf("error: couldn't change '%s'\n", first);
	}
	free(concat);
	free(origstore);
	showUsage = 0;
      }
    }
  }
  if (showUsage) printf("usage: salt <variable> <string...>\n");
}


void cmd_wifiqr(const int tty, const char *rest) {
  int ok = 0;
  if (rest) {
    char *copy = strdup(rest);
    char *pw = strtok(copy, " ");
    if (pw) {
      char *network = strtok(NULL, " ");
      if (network) {
	char s[1024];
	sprintf(s, "WIFI:S:%s;T:WPA2;P:%s;;", rest + (network - copy), pw);
	cmd_qr(tty, s);
	ok = 1;
      }
    }
    free(copy);
  }
  if (ok == 0) {
    printf("usage: wifiqr <password> <network>>\n");
  }
}

      

#include "tpmtotp/base32.h"

unsigned char * hmacKey = NULL;
size_t hmacKeyBytes = 10; // bytes

void cmd_authClear();
void cmd_authPrint(const int tty);

void cmd_authSet(const int tty, const char *keyorig, const size_t quiet) {
  if (tty) {}
  
  if (keyorig) {

    char *key = strdup(keyorig);
    for (size_t i = 0; i < strlen(key); i++) {
      if (islower(key[i])) key[i] = toupper(key[i]);
    }
    
    hmacKey = calloc(strlen(key), 1);
    hmacKeyBytes = base32_decode((unsigned char*)key, hmacKey);
    //    hmacKeyBytes = strlen((char*)hmacKey);
    if (hmacKeyBytes < 10) {
      cmd_authClear();
    }
    if (quiet == 0) cmd_authPrint(tty);
    free(key);
  } else {
    printf("usage: authset <key>\n");
  }
}


size_t cmd_authFromENV(const int tty, const char *username, const size_t quiet) { // returns adminMode
  cmd_authClear();

  size_t adminMode = 1;
  
  char s[1024];
  sprintf(s, "ADMIN_TOTP_USER_%s", username);
  for (size_t i =0; i < strlen(s); i++) {
    if (islower(s[i])) {
      s[i]=toupper(s[i]);
    }
  }
  
  if (getenv(s)) {
    //    printf("Admin/enable required for %s\n", username);
    adminMode = 0; // needs a TOTP to become admin

    cmd_authSet(tty, getenv(s), quiet);
  }
  if (quiet == 0) cmd_authPrint(tty);

  return adminMode;
}


void cmd_authPrint(const int tty) {
  if (tty) {}
  if (hmacKey) {
    printf("HMAC size: %zd bytes (%zd bits)\n", hmacKeyBytes, hmacKeyBytes * 8);
    uint8_t *p = (uint8_t*)hmacKey;
    printf("RAW random: ");
    for (size_t i = 0; i < hmacKeyBytes; i++) {
      printf("%02x ", *p);
      p++;
    }
    printf("\n");
    const size_t codedSize = (int)(ceil(hmacKeyBytes * 8.0 / 5));
    printf("base32 size: %zd bytes\n", codedSize);
    unsigned char coded[1+codedSize];
    memset(coded, 0, 1+codedSize);
    base32_encode((unsigned char*)hmacKey, hmacKeyBytes, coded);
    coded[codedSize] = 0;
    if (tty) printf("%s", BOLD);
    printf("key: '%s'\n", coded);
    if (tty) printf("%s", END);
  } else {
    printf("no auth setup\n");
  }
}

void cmd_authClear() {
  if (hmacKey) {
    free(hmacKey);
    hmacKey = NULL;
  }
}

void cmd_calcEntropy(const int tty, unsigned char *second);


void cmd_authGen(const int tty, const char *rest) {
  cmd_authClear();

  // the argument is in bits
  int bits = 80; // default with no args is 80
  if (rest) {
    bits = MAX(80, atoi(rest));
  }
  
  hmacKeyBytes = ceil (bits / 8.0); // 80 bits, 10 bytes by default

  hmacKey = randomGenerate(hmacKeyBytes);
  double entropy = entropyTotalBits(hmacKey, hmacKeyBytes, 1);
  printf("Shannon entropy: %.0lf bits\n", entropy);
  
  cmd_authPrint(tty);
}

void cmd_authQR(const int tty, const char *username, const char *hostname) {
  if (tty) {}

  if(hmacKey) {
    
    const size_t codedSize = (int)(ceil(hmacKeyBytes * 8.0 / 5));
    unsigned char coded[1+codedSize];
    memset(coded, 0, codedSize);
    base32_encode((unsigned char*)hmacKey, hmacKeyBytes, coded);
    coded[codedSize] = 0;
    
    
    char s[1024];
    sprintf(s, "otpauth://totp/%s@%s-%zd-bits?secret=%s&issuer=stush", username, hostname, hmacKeyBytes * 8, coded);
    cmd_qr(tty, s);
  } else {
    printf("no auth setup\n");
  }
}

#include "TOTP-MCU/TOTP.h"


uint32_t cmd_generateTOTP(const int tty, const int N, const int quiet) {
  if (tty) {}

  uint32_t newCode = 0, firstCode = 0;
  if (hmacKey) {

    long start = 30*(long)(time(NULL)/30);
    for (long i = start; i < start + (30*N) ; i+=30) {
      
      TOTP(hmacKey, hmacKeyBytes, 30); 
      
      setTimezone(9);        // set timezone used
      newCode = getCodeFromTimestamp(i);
      if (i == start) {
	firstCode = newCode;
      }

      char timestring[PATH_MAX];
      struct tm tm = *localtime(&i);
      strftime(timestring, 999, "%c %Z", &tm);
      
      if (quiet == 0) printf("%s: \t%06d%s\n", timestring, newCode, (i==start)?"\t* now":"");
    }
  }
  //  printf("%06u\n", newCode);
  return firstCode;
}
  


void cmd_sleep(const int tty, const char *second) {
  if (tty) {}

  double spectime = -1;

  if (second) { // sleep
    spectime = atof(second);
  }

  if (spectime < 0) {
    spectime = getRandomDouble(3.0);
  }

  char s[1000];
  sprintf(s, "sleeping for %.3lf seconds", spectime);
  syslogString("stush", s);
  
  usleep(spectime * 1000000L);
}

void cmd_who(const int tty) {
  if (tty) {}
  who(0);
}


double usedMemoryPercent() {
  size_t total = totalRAM();
  size_t free = freeRAM();
  size_t used = total - free;
  return used * 100.0 / total;
}
  


void cmd_memUsage(const int tty) {
  if (tty) {}
  printf("Memory usage: %.0lf%%\n", usedMemoryPercent());
}

void cmd_swapUsage(const int tty) {
  if (tty) {}

  size_t total, used;
  swapTotal(&total, &used);

  if (total == 0) {
    printf("Swap disabled\n");
  } else {
    printf("Swap usage: %.0lf%%\n", used * 100.0 / total);
  }
}



void cmd_id(const int tty) {
  if (tty) {}
  printf("process id:  %d (%s)\n", getpid(), getcmdline(getpid()));
  printf("parent pid:  %d (%s)\n", getppid(), getcmdline(getppid()));
  printf("ttyname:     %s\n", ttyname(1));
}

void cmd_testdns(const int tty, const char *second, dnsServersType *d) {
  if (tty) {}
  char hostname[100];

  // otherwise continue and test, adding in some standard ones
  const size_t N = dnsServersN(d);

  numListType *lookup = calloc(N, sizeof(numListType)); assert(lookup);

  for (size_t i =0 ;i < N; i++) {
    nlInit(&lookup[i], 10000);
  }
  
  for (size_t iterations = 0; keepRunning && iterations < 100; iterations++) {

    for (size_t i = 0; keepRunning && i < N; i++) if (d->dnsServer[i]) {

    unsigned char *rand = randomGenerate(3*8);

    unsigned char *pw = passwordGenerateString(rand, 3, "abcdefghijklmnopqrstuvwxyz");
    sprintf(hostname, "%s.com", pw);

    free(rand);
    free(pw);
  
    //  sprintf(hostname, "example.com");
    if (second) {
      sprintf(hostname, "%s", second);
    }


	
      printf("dns server: %s\n", d->dnsServer[i]);
    
      double start = timeAsDouble();
      int ret = dnsLookupAServer(hostname, d->dnsServer[i]);
      double end = timeAsDouble();
      if (ret > 0) {
	nlAdd(&lookup[i], 1000.0 * (end - start));
      }
    }
  }

  for (size_t i =0 ;i < N; i++) {
    printf("dns server: %-12s\t%zd\tmean %6.2lf ms\tmedian %6.1lf ms\n", d->dnsServer[i], nlN(&lookup[i]), nlMean(&lookup[i]), nlMedian(&lookup[i]));
  }
  for (size_t i =0 ;i < N; i++) {
    nlFree(&lookup[i]);
  }
  free(lookup);
}  
  
void cmd_last(const int tty) {
  if (tty) {}
  last(0);
}

void cmd_top(const int tty) {
  if (tty) {}
  listRunningProcessses();
}

void cmd_uptime(const int tty) {
  if (tty) {}
  time_t t = time(NULL);
  struct tm tm = *localtime(&t);
  
  char timestring[PATH_MAX];
  strftime(timestring, 999, " %H:%M:%S", &tm);

  t = getUptime();
  time_t d,h,s;
  timeToDHS(t, &d, &h, &s);
  
  double d1,d2,d3;
  loadAverage3(&d1, &d2, &d3);
  printf("%s up %zd days, %2zd:%02zd, %2zd user%s load average: %.2lf, %.2lf, %.2lf\n", timestring, d, h, s, who(1), who(1)>1?"s,":", ", d1, d2, d3);
}

void cmd_devSpeed(const int tty, const char *second, int showspeedorlatency) {
    if (second) {
      int fd = open(second, O_RDONLY | O_DIRECT);
      if (fd < 0) {
	perror(second);
      } else {
	unsigned int major, minor;
	if (majorAndMinor(fd, &major, &minor) != 0) {
	  printf("*warning* can't get major:minor for '%s'\n", second);
	}
	
	numListType speed, latency;
	
	nlInit(&speed, 1000000); 
	nlInit(&latency, 1000000);
	
	size_t len = 2L * 1024 * 1024;
	if (showspeedorlatency == 0) {
	  len = 4096;
	}
	
	if (showspeedorlatency) {
	  if (tty) printf("%s", BOLD);
	  fprintf(stdout, "*info* sequential speed '%s', size=%zd for 5 seconds (MB/s)\n", second, len);
	  if (tty) printf("%s", END);
	} else {
	  if (tty) printf("%s", BOLD);
	  fprintf(stdout, "*info* sequential latency '%s', size=%zd for 5 seconds (µs)\n", second, len);
	  if (tty) printf("%s", END);
	}
	// do test
	readBenchmark(fd, 5, len, &speed, &latency);
	
	if (showspeedorlatency) {
	  nlSummary(&speed);
	} else {
	  nlSummary(&latency);
	}
	  
	nlFree(&speed);
	nlFree(&latency);
	close(fd);
      }
    } else {
      printf("%s: dev%s <device>\n", T("usage"), (showspeedorlatency?"speed":"latency"));
    }
}  

void cmd_numProcesses(const int tty) {
  if (tty) {}
  size_t np = numberOfDirectories("/proc");
  printf("%zd\n", np);
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


void cmd_calcEntropy(const int tty, unsigned char *second) {
  if (second) {
    double entropy = entropyTotalBits(second, strlen((char*)second), 1);
    printf("%s ", second);
    char ss[PATH_MAX];
    sprintf(ss, "(%.1lf bits of entropy)", entropy);
    colour_printString(ss, entropy >= 200, "\n", tty);
  } else {
    printf("%s: %s <string>\n", T("Usage"), "entropy");
  }
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

#include "failureType.h"

void cmd_raidsim(int tty, char *origline) {
  if (tty) {}

  char *s = strdup(origline);
  int ran = 0;
  char *first = strtok(s, " ");
  if (first) {
    char *second = strtok(NULL, " ");
    if (second) {
      char *third = strtok(NULL, " ");
      if (third) {
	char *four = strtok(NULL, " ");
	if (four) {
	  char *arg[]={first, second, third, four};
	  ran = 1;
	  failureType(4, arg);
	}
      }
    }
  }
  free(s);
  if (ran == 0) {
    printf("%s: raidsim <k> <m> <LHS>\n", T("usage"));
    printf("\n%s:: \n", T("Examples"));
    printf("  raidsim 6 2 0      --- 8 drives in RAID6/2-parity with 0 spares\n");
  }
}

void cmd_listPCI(int tty, size_t filterclass, char *label) {
  if (tty) printf("%s", BOLD);
  printf("%s\n", label);
  if (tty) printf("%s", END);
  
  listPCIdevices(filterclass);
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
	    //            dumpFile(ss, "", 0);
	    double linkup = getValueFromFile(ss, 1);
	    printf("%s\n", linkup>=1 ? "UP" : "DOWN");

            printf("   Speed: ");
            sprintf(ss, "/sys/class/net/%s/speed", ifa->ifa_name);
	    double speed = getValueFromFile(ss, 1);
	    printf("%.0lf\n", speed);

            printf("   MTU: ");
            sprintf(ss, "/sys/class/net/%s/mtu", ifa->ifa_name);
	    double mtu = getValueFromFile(ss, 1);
	    printf("%.0lf\n", mtu);
	    //            dumpFile(ss, "", 0);

            printf("   Carrier changes: ");
            sprintf(ss, "/sys/class/net/%s/carrier_changes", ifa->ifa_name);
	    double cc = getValueFromFile(ss, 1);
	    printf("%.0lf\n", cc);
	    //            dumpFile(ss, "", 0);


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


// from getifaddrs man page
void cmd_listNICs2(int tty) {
    if (tty) {}

    struct ifaddrs *ifaddr;
    int family, s;
    char host[NI_MAXHOST];

    if (getifaddrs(&ifaddr) == -1) {
        perror("getifaddrs");
        exit(EXIT_FAILURE);
    }

    if (tty) printf("%s", BOLD);
    printf("if    \tIPv4         \tHW/MAC           \tLINK  \tSPEED\tMTU\tChanges\n");
    if (tty) printf("%s", END);

    for (struct ifaddrs *ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
        if (ifa->ifa_addr == NULL)
            continue;

        family = ifa->ifa_addr->sa_family;

        /* Display interface name and family (including symbolic
           form of the latter for the common families). */

        if (family == AF_INET/* || family == AF_INET6*/) {
	  if (strcmp(ifa->ifa_name, "lo")==0) {
	    continue; // don't print info on localhost
	  }
	  
	  printf("%s\t", ifa->ifa_name);

	  s = getnameinfo(ifa->ifa_addr,
                            (family == AF_INET) ? sizeof(struct sockaddr_in) :
                            sizeof(struct sockaddr_in6),
                            host, NI_MAXHOST,
                            NULL, 0, NI_NUMERICHOST);
            if (s != 0) {
                printf("getnameinfo() failed: %s\n", gai_strerror(s));
                exit(EXIT_FAILURE);
            }
	    
            printf("%s\t", host);
            cmd_printHWAddr(ifa->ifa_name); printf("\t");

            char ss[PATH_MAX];
            sprintf(ss, "/sys/class/net/%s/carrier", ifa->ifa_name);
	    //            dumpFile(ss, "", 0);
	    double linkup = getValueFromFile(ss, 1);
	    printf("%-6s", linkup>=1 ? "UP" : "DOWN");
	    printf("\t");

            sprintf(ss, "/sys/class/net/%s/speed", ifa->ifa_name);
	    double speed = getValueFromFile(ss, 1);
	    printf("%.0lf\t", speed);

            sprintf(ss, "/sys/class/net/%s/mtu", ifa->ifa_name);
	    double mtu = getValueFromFile(ss, 1);
	    printf("%.0lf\t", mtu);

            sprintf(ss, "/sys/class/net/%s/carrier_changes", ifa->ifa_name);
	    double cc = getValueFromFile(ss, 1);
	    printf("%.0lf\n", cc);
        }
    }

    freeifaddrs(ifaddr);
}

void cmd_listAll(const size_t adminMode) {
    printf("%s: \n", T("Commands"));
    for (size_t i = 0; i < sizeof(commands) / sizeof(commands[0]); i++) {
      size_t show = 1;
      if (adminMode) {} // same as default
      else {
	// not root
	if (strcmp(commands[i].admin, "admin")==0) show = 0;
      }

      if (show) {
	printf("  %-10s \t| %s\n", commands[i].name, T(commands[i].doc));
      }
    }
}

// 16 is the minimum
void cmd_pwgen(int tty, const char *second) {
    size_t len = 16, targetbits = 200;

    if (second) {
      targetbits = atoi(second);
    }

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
    printf("%s %zd: ", T("generate random bits for length"), len);
    if (tty) printf("%s", END);

    sprintf(ss, "(%.1lf %s)", bitsentropy, T("bits of entropy"));
    colour_printString(ss, bitsentropy >= 200, "\n", tty);


    printf("%s ", pw);
    sprintf(ss, "(%.1lf %s, %.2lf bpc)", pwentropy, T("bits of entropy"), pwentropy / len);
    colour_printString(ss, pwentropy >= 200, "\n", tty);
    if (found == 0) {
        if (tty) printf("%s", BOLD);
        printf("%s\n", T("password is weak and is below complexity target"));
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
            printf("   %s[%3zd] =  %3zd '%s'\n", T("Count"), i, majCount[i], majString?majString:"");
            free(majString);
        }
    }
    procDiskStatsFree(&d);
    return count;
}


void cmd_listDriveBlockDevices(int tty, const char *second) {
    if (tty) {}
    size_t major = 0;

    if (second) {
      major = atoi(second);
    }

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

void cmd_df(const int tty, const char *second) {
    if (tty) {}	
    if (second) {
      diskSpaceFromMount(second, T("Mount point"), T("Total (GB)"), T("Free (GB)"), T("Use%"));
    } else {
      printf("%s: df <%s>\n", T("usage"), T("mount point"));
    }
}

void cmd_scsi(const int tty) {
    if (tty) {}
    dumpFile("/proc/scsi/scsi", "", 0);
}

void cmd_netserver(const int tty) {
  if (tty) {}
  snackServer(10, 5001);
}

void cmd_netclient(const int tty, const char *second) {
  if (tty) {}

  if (second) {
    size_t len = 1024*1024;

    char *temp = strdup(second);
    char *third = strtok(temp, " \t");
    if (third) {
      len = atoi(third);
    }
    free(temp);
    
    snackClient(second, len, 5001, 10);
  } else {
    printf("%s: netclient <ipaddress> [len] (on ports [5001-5010]\n", T("usage"));
  }
}


void cmd_ntp(const int tty) {
  if (tty) {}
  
  struct ntptimeval ntv;
  ntp_gettime(&ntv);

  printf("Estimated error:          %.3lf s\n", ntv.esterror/1000000.0);
  printf("Maximum error:            %.3lf s\n", ntv.maxerror/1000000.0);
  printf("TAI (Atomic Time Offset): %.3lf\n", ntv.tai/1000000.0);
}
  
void cmd_time(const int tty, const double timeSinceStart) {
  if (tty) {}
  time_t tz = time(NULL);
  struct tm tm = *localtime(&tz);
  char timestring[PATH_MAX];
  strftime(timestring, 999, " %Z", &tm); // just want the time zone

  char *os = OSRelease();

  double t = timeOnPPS();
  
  printf("%lf %s %lf %s\n", t, timestring, t - timeSinceStart, os);
  fflush(stdout);
  free(os);
}

void cmd_timeAudit(const int tty, const double timeSinceStart, const char *rest) {
  if (tty) {}
  if (timeSinceStart) {}

  if (rest) {
    char *endp;
    double incomingTime = strtoul(rest, &endp, 10);
    if (endp) {
      if (*endp == ' ' || *endp == 0) {
	// ok
      } else {
	printf("error: not valid syntax of <time> <server details> ('%s')\n", rest);
	return;
      }
    }
    double now = timeAsDouble(); // now - time start
    double slowDown = now - timeSinceStart; // seconds of slow to get here
    if (slowDown < 1) { // if it takes less than a sec to start, then use it
      if (fabs(now - incomingTime) < 2) { // 2 secs max difference else log it
	char s[1024];
	sprintf(s, "time audit accurate: %lf - %s", now, rest);
	syslogString("stush", s);
	printf("%s\n", s);
	
	logStringToFile("/var/log/timeaudit.log", s);
	
      } else {
	char s[1024];
	sprintf(s, "**WARNING** time is out of sync: %lf delta - %s", now - incomingTime, rest);
	printf("%s\n", s);
	syslogString("stush", s);

	logStringToFile("/var/log/timeaudit.log", s);

      }
    } else {
      printf("**warning** ignoring as server is slow to respond\n");
    }
  } else {
    printf("usage: timeaudit <time> <calling server details>\n");
  }
}



void cmd_date(const int tty) {
    if (tty) {}
    time_t t = time(NULL);
    struct tm tm = *localtime(&t);

    char timestring[PATH_MAX];
    strftime(timestring, 999, "%c", &tm);

    printf("%s\n", timestring);
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

// for the ssh/logout alarm()
size_t ssh_timeout = 0;

void setSSHTimeout(const size_t t, const size_t quiet) {
  ssh_timeout = t;
  if (quiet == 0) printf("timeout: %zd\n", ssh_timeout);
}

size_t getSSHTimeout() {
  return ssh_timeout;
}


int run_command(const int tty, char *line, const char *username, const char *hostname, const int ssh_login, const double timeSinceStart, dnsServersType *dns, const size_t adminMode) {
    if (ssh_login) {}
    
    int known = 0;
    for (size_t i = 0; i < sizeof(commands) / sizeof(COMMAND); i++) {

        if ((adminMode == 0) && (strcasecmp(commands[i].admin, "admin")==0)) continue; // skip admin commands
       
        if (commands[i].name && (strncmp(line, commands[i].name, strlen(commands[i].name)) == 0)) {

	    char *copy = strdup(line);
	    char *first = strtok(copy, " ");
	    char *rest = NULL;
	    if (first) {
	      char *second = strtok(NULL, " ");

	      if (second) {
		rest = line + (second - first);
	      }
	    }
	  
            if (strcasecmp(commands[i].name, "status") == 0) {
                cmd_status(hostname, tty);
            } else if (strcasecmp(commands[i].name, "timeout") == 0) {
	      printf("%zd\n", getSSHTimeout());
	    } else if (strcasecmp(commands[i].name, "translations") == 0) {
	      cmd_translations(tty);
            } else if (strcasecmp(commands[i].name, "pwgen") == 0) {
                cmd_pwgen(tty, rest);
            } else if (strcasecmp(commands[i].name, "lsblk") == 0) {
                cmd_listDriveBlockDevices(tty, rest);
            } else if (strcasecmp(commands[i].name, "raidsim") == 0) {
	        cmd_raidsim(tty, line); // needs the whole line
            } else if (strcasecmp(commands[i].name, "entropy") == 0) {
	      cmd_calcEntropy(tty, (unsigned char*) rest);
            } else if (strcasecmp(commands[i].name, "cpu") == 0) {
                cmd_cpu(tty);
            } else if (strcasecmp(commands[i].name, "dropbear") == 0) {
                cmd_dropbear(tty);
            } else if (strcasecmp(commands[i].name, "mounts") == 0) {
                cmd_mounts(tty);
            } else if (strcasecmp(commands[i].name, "scsi") == 0) {
                cmd_scsi(tty);
            } else if (strcasecmp(commands[i].name, "spit") == 0) {
   	        cmd_spit(tty, line); // needs whole line
            } else if (strcasecmp(commands[i].name, "ps") == 0) {
	      cmd_numProcesses(tty);
            } else if (strcasecmp(commands[i].name, "df") == 0) {
                cmd_df(tty, rest);
            } else if (strcasecmp(commands[i].name, "date") == 0) {
                cmd_date(tty);
            } else if (strcasecmp(commands[i].name, "ntp") == 0) {
  	        cmd_ntp(tty);
            } else if (strcasecmp(commands[i].name, "netserver") == 0) {
                cmd_netserver(tty);
            } else if (strcasecmp(commands[i].name, "netclient") == 0) {
	        cmd_netclient(tty, rest);
            } else if (strcasecmp(commands[i].name, "lang") == 0) {
	        cmd_lang(tty, rest, 0);
		help_prompt();
            } else if (strcasecmp(commands[i].name, "tty") == 0) {
	        cmd_tty(tty);
	    } else if (strcasecmp(commands[i].name, "env") == 0) {
	        cmd_env(tty);
	    } else if (strcasecmp(commands[i].name, "setenv") == 0)  {
		cmd_setenv(tty, rest);
            } else if (strcasecmp(commands[i].name, "lsnic") == 0) {
                cmd_listNICs(tty);
            } else if (strcasecmp(commands[i].name, "ip") == 0) {
                cmd_listNICs2(tty);
            } else if (strcasecmp(commands[i].name, "lspci") == 0) {
	      cmd_listPCI(tty, 0x0200, "Networking");
	      cmd_listPCI(tty, 0x0100, "Storage");
	      cmd_listPCI(tty, 0x0300, "Video");
            } else if (strcasecmp(commands[i].name, "devspeed") == 0) {
	        cmd_devSpeed(tty, rest, 1);
            } else if (strcasecmp(commands[i].name, "who") == 0) {
	      cmd_who(tty); 
            } else if (strcasecmp(commands[i].name, "mem") == 0) {
	      cmd_memUsage(tty); 
            } else if (strcasecmp(commands[i].name, "swap") == 0) {
	      cmd_swapUsage(tty); 
            } else if (strcasecmp(commands[i].name, "last") == 0) {
	      cmd_last(tty); 
            } else if (strcasecmp(commands[i].name, "hexset") == 0) {
	      cmd_hexset(tty, rest); 
            } else if (strcasecmp(commands[i].name, "top") == 0) {
	      cmd_top(tty);
            } else if (strcasecmp(commands[i].name, "id") == 0) {
	      cmd_id(tty);
            } else if (strcasecmp(commands[i].name, "dnsls") == 0) {
	      dnsServersDump(dns);
            } else if (strcasecmp(commands[i].name, "dnsrc") == 0) {
	      dnsServersAddFile(dns, "/etc/resolv.conf", "nameserver");
	      dnsServersDump(dns);
            } else if (strcasecmp(commands[i].name, "dnsclear") == 0) {
	      dnsServersClear(dns);
            } else if (strcasecmp(commands[i].name, "dnsrm") == 0) {
	      if (rest) {
		dnsServersRm(dns, atoi(rest));
	      }
	      dnsServersDump(dns);
            } else if (strcasecmp(commands[i].name, "dnsadd") == 0) {
	      if (rest) {
		dnsServersAdd(dns, rest);
	      }
            } else if (strcasecmp(commands[i].name, "dnstest") == 0) {
	      cmd_testdns(tty, rest, dns);
            } else if (strcasecmp(commands[i].name, "timeaudit") == 0) {
	      cmd_timeAudit(tty, timeSinceStart, rest);
            } else if (strcasecmp(commands[i].name, "time") == 0) {
	      cmd_time(tty, timeSinceStart);
            } else if (strcasecmp(commands[i].name, "sleep") == 0) {
	      cmd_sleep(tty, rest);
            } else if (strcasecmp(commands[i].name, "host") == 0) {
	      dnsLookupAll(dns, rest); 
            } else if (strcasecmp(commands[i].name, "uptime") == 0) {
	      cmd_uptime(tty);
            } else if (strcasecmp(commands[i].name, "qr") == 0) {
	      cmd_qr(tty, rest);
            } else if (strcasecmp(commands[i].name, "hexascii") == 0) {
	      cmd_encodeASCII(tty, rest);
            } else if (strcasecmp(commands[i].name, "hexdump") == 0) {
	      cmd_hexdump(tty, rest);
            } else if (strcasecmp(commands[i].name, "hexstring") == 0) {
	      cmd_hexstring(tty, rest);
            } else if (strcasecmp(commands[i].name, "salt") == 0) {
	      cmd_addSalt(tty, rest);
            } else if (strcasecmp(commands[i].name, "wifiqr") == 0) {
	      cmd_wifiqr(tty, rest);
            } else if (strcasecmp(commands[i].name, "authgen") == 0) {
	      cmd_authGen(tty, rest);
            } else if (strcasecmp(commands[i].name, "authls") == 0) {
	      cmd_authPrint(tty);
            } else if (strcasecmp(commands[i].name, "authclear") == 0) {
	      cmd_authClear(tty);
	    } else if (strcasecmp(commands[i].name, "authqr") == 0) {
	      cmd_authQR(tty, username, hostname);
	    } else if (strcasecmp(commands[i].name, "authset") == 0) {
	      cmd_authSet(tty, rest, 0);
	    } else if (strcasecmp(commands[i].name, "authenv") == 0) {
	      cmd_authFromENV(tty, username, 0);
            } else if (strcasecmp(commands[i].name, "authtok") == 0) {
	      if (hmacKey) {
		int n = 1;
		if (rest) n = atoi(rest);
		if (n < 1) n = 1;
		if (rest) {
		  cmd_generateTOTP(tty, n, 0);
		} else {
		  int t = cmd_generateTOTP(tty, n, 1);
		  printf("%06u\n", t);
		}
	      }
            } else if (strcasecmp(commands[i].name, "route") == 0) {
	      cmd_route();
	    } else if (strcasecmp(commands[i].name, "h2d") == 0) {
	      if (rest) {
		unsigned long n = strtoul(rest, NULL, 16);
		printf("%ld\n", n);
		unsigned long n2 = n;
		volatile unsigned char *p = (unsigned char*)&n2;
		printf("%d.%d.%d.%d\n", p[0],p[1],p[2],p[3]);
		printBinary(p[0], 8); printf(".");
		printBinary(p[1], 8); printf(".");
		printBinary(p[2], 8); printf(".");
		printBinary(p[3], 8); printf("\n");
		printBinary(n2, 64); printf("\n");
	      }
            } else if (strcasecmp(commands[i].name, "d2h") == 0) {
	      if (rest) {
		unsigned long n = strtoul(rest, NULL, 10);
		printf("%lu\n", n);
		printf("0x%lX\n", n);
		printBinary(n, 64); printf("\n");
	      }
            } else if (strcasecmp(commands[i].name, "d2b") == 0) {
	      if (rest) {
		unsigned long n = strtoul(rest, NULL, 10);
		if (errno == ERANGE) {
		  printf("number too big\n");
		} 
		printBinary(n, 64);
		printf("\n");
	      }
            } else if (strcasecmp(commands[i].name, "ascii") == 0) {
	      if (rest) {
		unsigned char cc= rest[0];
		printf("'%c',  '%d'  ", cc, cc);
		printBinary(cc, 8);
		printf("\n");
	      }
	    } else if (strcasecmp(commands[i].name, "devlatency") == 0) {
	      cmd_devSpeed(tty, line, 0);
	    }
            known = 1;
	    free(copy);

	    break;
	}
    }
    if ((strcasecmp(line, "?") == 0) || (strcasecmp(line, "help") == 0)) {
        cmd_listAll(adminMode);
    } else {
        if (!known) {
  	    printf("%s: %s\n", line, T("Unknown command"));
        }
    }
    return !known;
}

int main(int argc, char *argv[]) {
    const double timeSinceStart = timeAsDouble();
  
    syslogString("stush", "Start session");
    if (geteuid() != 0) {
        fprintf(stderr, "*error* app needs root. sudo chmod +s ...\n");
        syslogString("stush", "error. app needs root.");
        exit(1);
    }

    // ENV load
    char *username = getenv("USER");
    if (username) username = strdup(username);

    //
    int tty = isatty(1);

    //
    loadEnvVars("/etc/stush.cfg");
    size_t adminMode = cmd_authFromENV(tty, username, 1);

    // DNS load
    dnsServersType dns;
    dnsServersInit(&dns);
    dnsServersAddFile(&dns, "/etc/resolv.conf", "nameserver");



    const size_t ssh_login = isSSHLogin();

    if (ssh_login) {
      syslogString("stush", getenv("SSH_CLIENT"));
      syslogString("stush", "SSH_TIMEOUT");
      if (getenv("SSH_TIMEOUT")) {
	setSSHTimeout(atoi(getenv("SSH_TIMEOUT")), 1);
	syslogString("stush", getenv("SSH_TIMEOUT"));
      } else {
	setSSHTimeout(180, 1);
	syslogString("stush", "180"); // default is 180
      }
    }


    signal(SIGTERM, intHandler);
    signal(SIGINT, intHandler);
    signal(SIGALRM, intHandler);
    setvbuf(stdout, NULL, _IONBF, 0);  // turn off buffering
    setvbuf(stderr, NULL, _IONBF, 0);  // turn off buffering


    char hostname[NAME_MAX - 10], prefix[NAME_MAX];
    if (gethostname(hostname, NAME_MAX - 11)) {
        sprintf(hostname, "stush");
    }


    // cli
    size_t runArg = 0;
    for (int i = 1; i < argc; i++) if (strlen(argv[i]) < 100) { // no crazy commands
        if (argv[i][0] != '-') {
	    char s[1024];
	    char *copy = strdup(argv[i]);
	    char *save;
	    char *tok = strtok_r(copy, ";", &save); // split on ; only
	    size_t count = 0;
	    do {
	      count++;
	      if (tok) {
		sprintf(s, "argv[%d.%zd] = '%s'", i, count, tok);
		syslogString("stush", s);
		int ret = run_command(tty, tok, username, hostname, ssh_login, timeSinceStart, &dns, adminMode);
		if (ret) {
		  sprintf(s, "'%s' -- command failed", tok);
		  syslogString("stush", s);
		}
		runArg = 1;
		tok = strtok_r(NULL, ";", &save);
	      }
	    } while (tok);
	    free(copy);
        }
    }
    if (runArg) goto end;


    funHeader();
    header(tty);
    cmd_status(hostname, tty);

    help_prompt();

    char *line = NULL;
    rl_bind_key('\t', rl_insert);

    while (1) {
      sprintf(prefix, "%s%s@%s%c%s ", tty?BOLD:"", username ? username : "root", hostname, adminMode ? '#' : '$', tty?END:"");
        keepRunning = 1;

	if (ssh_login) {alarm(getSSHTimeout());}
        line = readline(prefix);
	if (ssh_login) {alarm(0);}

	if (line && strlen(line)>=100) {
	  free(line);
	  line = NULL;
	  continue; // no crazy commands
	}

	if (adminMode == 0) { // user mode
	  if (line && (strcasecmp(line, "admin")==0)) {
	    sprintf(prefix, "auth token# ");
	    line = readline(prefix);
	    int t = cmd_generateTOTP(tty, 1, 1);
	    if (atoi(line) == t) {
	      syslogString("stush", "[promoted to admin]");
	      printf("[promoted to admin]\n");
	      adminMode = 1;
	      setSSHTimeout(0, 0); // don't kick an admin out ever, and don't be quiet, let them know
	      free(line);
	      line = NULL;
	      continue;
	    } else {
	      syslogString("stush", "incorrect enable password");
	      sleep(2);
	      printf("error: incorrect\n");
	      continue;
	    }
	  }
	}
	  
        if ((line == NULL) || (strcasecmp(line, "exit") == 0) || (strcasecmp(line, "quit") == 0)) {
            break;
        }
	
        if (strlen(line) < 1) {
            free(line);
	    line = NULL;
            continue;
        }
        syslogString("stush", line); // log


        add_history(line);

        int ret = run_command(tty, line, username, hostname, ssh_login, timeSinceStart, &dns, adminMode);
	if (ret) {
	  syslogString("stush", "-- command failed");
	}
    }

    //    if (line == NULL) printf("\n");

    if (tty) printf("%s", BOLD);
    printf("%s\n", T("Goodbye"));
    if (tty) printf("%s", END);

    free(line);

 end:
    free(username);

    clear_history();
    syslogString("stush", "Close session");
    dnsServersFree(&dns);

    return 0;
}
