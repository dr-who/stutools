#include <stdio.h>
#include <malloc.h>
#include <string.h>
#include <stdlib.h>
#include <getopt.h>
#include <unistd.h>

#include "simpmail.h"
#include "emaildb.h"

// returns the number of lines
char *readFile(FILE *stream) {

  char *ret = calloc(1024*1024, 1);
  
    if (stream) {
        char *line = NULL;
        size_t linelen = 0;
        ssize_t nread;

        while ((nread = getline(&line, &linelen, stream)) != -1) {
            //      printf("Retrieved line of length %zu:\n", nread);

	  //	  len = len + (nread + 1);
	  
	  //	  ret = realloc(ret, len * sizeof(char));
	  
	  strcat(ret, line);
	  //	  strcat(ret, "\n");
        }

        free(line);
    } else {
        perror("problem");
    }

    return ret;
}

void usage() {
  printf("usage: simpmail -f <from> -s <subject> [ -t toemail | -l list.txt ]   -p file.html \n");
  printf("\nOptions:\n");
  printf("  -t email         # to email address (singular)\n");
  printf("  -l list.txt      # to a list of email addresses (one per line)\n");
  printf("\n");
  printf("  -d               # dry run, don't send emails\n");
  printf("\n");
  printf("  -p payload/body  # body filename\n");
  printf("  -s subject       # subject (use \"'s)\n");
  printf("  -f email         # from email address\n");
  printf("  -F name          # plain name (use \"'s)\n");
  printf("\n");
  printf("  -c email         # cc email address (singular)\n");
  printf("  -b email         # bcc email address (singular)\n");
  printf("  -r rate/sec      # defaults to 20 (sleep 1/20 after each)\n");
  printf("\nExamples:\n");

  printf("  simpmail -t test@example.com -f bob@example.com -F \"Bob Hope\" -c cc@example.com -s \"Test subject in quotes\" -p file.html -d # -d dryrun\n");
  printf("\n");
  printf("  simpmail -l big-ema-list.txt -f bob@example.com -F \"Bob Hope\" -c cc@example.com -s \"Test subject in quotes\" -p file.html -d # -d dryrun\n");

    printf("\nHTML example:\n");
  printf("  Look in 'simpmail-template.html'\n");
}

int main(int argc, char *argv[]) {

  int opt;
  int dryRun = 0;
  char *fromemail = NULL, *fromname = NULL, *toemail = NULL, *ccemail = NULL, *bccemail = NULL, *subject = NULL, *payload = NULL;
  emaildbType *e = NULL;
  double ratepersecond = 20;
  
  while ((opt = getopt(argc, argv, "f:F:t:c:b:s:l:p:r:d")) != -1) {
    switch (opt) {
    case 'd':
      dryRun = 1;
      break;
    case 'r':
      ratepersecond = atof(optarg);
      break;
    case 'f':
      fromemail = strdup(optarg); break;
    case 'F':
      fromname = strdup(optarg); break;
    case 't':
      toemail = strdup(optarg); break;
    case 'c':
      ccemail = strdup(optarg); break;
    case 'b':
      bccemail = strdup(optarg); break;
    case 's':
      subject = strdup(optarg); break;
    case 'p':
      {
	fprintf(stderr,"*info* body/payload is '%s'\n", optarg);
	FILE *fp = fopen(optarg, "rt");
	if (fp) {
	  payload = readFile(fp);
	  fclose(fp);
	}
      }
      break;
    case 'l':
      {
      FILE *fp = fopen(optarg, "rt");
      if (fp) {
	e = emaildbLoad(fp);
      }
      fclose(fp);
      //      emaildbDump(e);
      //      emaildbFree(e);
      break;
      }
    default:
      fprintf(stderr,"unknown command\n");
      exit(EXIT_FAILURE);
    }
  }

  if (!fromemail || !subject || !payload) {
    usage();
    exit(1);
  }
  if (!toemail && !e) {
    usage();
    exit(1);
  }
    

  if (dryRun) {
    fprintf(stderr,"*warning* DRY RUN ONLY. NO EMAILS SENT\n");
  }
  fprintf(stderr,"*info* rate per second = %.1lf\n", ratepersecond);
  
  
  if (e) { // if an email database
    printf("*info* There are %zd unique email addresses\n", e->len);
    size_t next = 0;
    for (size_t i = 0; i < e->len; i++) {
      if (i >= next) {
	printf("Send how many [1,2 ... all] (%s)? ", e->addr[i]); fflush(stdout);
	char input[100];
	char buf[100];

	
	if (fgets(buf, sizeof buf, stdin) != NULL) {
	  int ss = sscanf(buf, "%s", input);
	  if (ss) {
	    if (strcmp(input, "all")==0) {
	      next = e->len;
	    } else {
	      char *ptr = NULL;
	      unsigned long gap = strtoul(input, &ptr, 10);
	      if (ptr != NULL) {
		next = i+ gap;
	      } else {
		next = i;
	      }
	    }
	  } else {
	    next=i;
	  }
	} else {
	  // eof
	  printf("*terminated\n");
	  break;
	}
      } // i>= next
	  
      // send the email
      if (dryRun) {
	printf("DRY From \"%s\" <%s>, To <%s>, CC <%s>, BCC <%s>, Payload %ld bytes, Subject \"%s\"\n", fromname?fromname:"", fromemail, e->addr[i], ccemail, bccemail, strlen(payload), subject);
      } else {
	int fd = simpmailConnect("127.0.0.1");
	
	if (fd > 0) {
	  simpmailSend(fd, i >= 10, fromemail, fromname, e->addr[i], ccemail, bccemail, subject, payload);
	  simpmailClose(fd);
	}
      }
      
      fprintf(stderr,"[%zd of %zd] %s\n", i+1, e->len, e->addr[i]);
      
      if (ratepersecond && (dryRun == 0)) {
	usleep(1000 * 1000 / ratepersecond); // a bit of a pause
      }

    } // iterate over all email addresses
    emaildbFree(e);
    
  } else { // a one-off

    if (dryRun) {
      printf("DRY From \"%s\" <%s>, To <%s>, CC <%s>, BCC <%s>, Payload %ld bytes, Subject \"%s\"\n", fromname?fromname:"", fromemail, toemail, ccemail, bccemail, strlen(payload), subject);
    } else {
      int fd = simpmailConnect("127.0.0.1");
      
      if (fd > 0) {
	simpmailSend(fd, 0, fromemail, fromname, toemail, ccemail, bccemail, subject, payload);
	simpmailClose(fd);
      }
    }
  } // one-off

  // cleanup
  
  if (payload) free(payload);
  if (fromemail) free(fromemail);
  if (fromname) free(fromname);
  if (toemail) free(toemail);
  if (ccemail) free(ccemail);
  if (bccemail) free(bccemail);
  if (subject) free(subject);


  return 0;
}

