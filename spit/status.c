#include "status.h"

#include <sys/sysinfo.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <malloc.h>
#include <sys/timex.h>

// returns the ASCII output, and error==0 means ok. error > 0 means error.

// if NTP isn't synced or free ram is under 0.2 GB then return an error.
char *genStatus(int *error) {
  *error = 0;

  // first section is the time
  struct timex timex_info = {};
  timex_info.modes = 0;

  int ntp_result = ntp_adjtime(&timex_info);

  char *s = calloc(1024*1024, 1); // big alloc, but doesn't last long
  char *rets = s;
  if (s) {
    char timestring[1000];
    long thetm = time(NULL);
    struct tm tm = *localtime(&thetm);
    strftime(timestring, 999, "%c %Z", &tm);
    s += sprintf(s, "%s\n", timestring);
    s += sprintf(s, "\n");

    
    s += sprintf(s, "Time max error:  %9ld (µs)\n",timex_info.maxerror);
    s += sprintf(s, "Time est error:  %9ld (µs)\n",timex_info.esterror);
    s += sprintf(s, "Clock precision: %9ld (µs)\n",timex_info.precision);
    s += sprintf(s, "Jitter:          %9ld (%s)\n",timex_info.jitter,
		 (timex_info.status & STA_NANO) ? "ns" : "µs");
    
    const int ntp_sync = (ntp_result >=0 && ntp_result != TIME_ERROR);
    if (ntp_sync != 1) {
      *error = 1;
    }
    s += sprintf(s, "NTP Synchronised %9s\n", ntp_sync ? "OK/YES" : "ERROR/NO");
    s += sprintf(s, "\n");
  }

  if (s) {
    struct sysinfo info;
    sysinfo(&info);
    s += sprintf(s, "Uptime:     %.1lf days\n", (info.uptime / 3600.0 / 24));
    s += sprintf(s, "\n");
    s += sprintf(s, "Shared RAM: %.1lf GiB\n", (info.sharedram / 1024.0 / 1024.0 / 1024.0));
    const float freeram_gib = (info.freeram / 1024.0 / 1024.0 / 1024.0);
    if (freeram_gib < 0.2) {
      *error = 1;
    }
    s += sprintf(s, "Free RAM:   %.1lf GiB (%s)\n", freeram_gib, (freeram_gib < 0.2) ? "ERROR/NO" : "OK/YES");
    s += sprintf(s, "Used RAM:   %.1lf GiB\n", (info.totalram - info.freeram) / 1024.0 / 1024.0 / 1024.0);
    s += sprintf(s, "Total RAM:  %.1lf GiB\n", (info.totalram / 1024.0 / 1024.0 / 1024.0));
    s += sprintf(s, "\n");
    s += sprintf(s, "Free Swap: %.1lf GB\n", (info.freeswap / 1000.0 / 1000.0 / 1000.0));
    s += sprintf(s, "Total Swap: %.1lf GB\n", (info.totalswap / 1000.0 / 1000.0 / 1000.0));
    s += sprintf(s, "\n");

    const float f_load = 1.f / (1 << SI_LOAD_SHIFT);
    s += sprintf(s, "Load average (1, 5, 15 mins): %.2f, %.2f %.2f\n",  info.loads[0] * f_load, info.loads[1] * f_load, info.loads[2] * f_load);
  }
  
  return rets;
}
    
