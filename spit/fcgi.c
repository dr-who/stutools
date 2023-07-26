#include "fcgi_stdio.h" /* fcgi library; put it first*/

#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <fcntl.h>
#include <stdio.h>
#include <time.h>

#include <sys/sendfile.h>

int startsWith(const char *pre, const char *str) {
    size_t lenpre = strlen(pre), lenstr = strlen(str);
    return lenstr < lenpre ? 0 : strncmp(pre, str, lenpre) == 0;
}


int count;

void initialize(void)
{
  count=0;
}

extern char **environ;

// api/customer?
// ?create
// ?delete
// ?email

void customerURL(char *url, char *query, char *api) {
  printf("<h1> %s %s %s </h1>\n", url, query, api);
}

// create
// delete
// o
void orderURL(char *url, char *query, char *api) {
  printf("<h1> %s %s %s </h1>\n", url, query, api);
  if (startsWith("list", query)) {
    printf("list all the orders\n");
  } else if (startsWith("add", query)) {
    printf("adds the orders\n");
  } else if (startsWith("view", query)) {
    printf("view one order\n");
  } else if (startsWith("delete", query)) {
    printf("delete one order\n");
  }
}
  
void getFile(const char *fn) {
  FILE *fp = fopen(fn, "rt");
  if (fp) {
    printf("\r\n"); // header finished
    char *line = NULL;
    size_t len = 0;
    ssize_t nread;
    const size_t size = 100*1024;
    char *buffer = malloc(size); 
    int r = 0;

    while ((r = fread(buffer, 1, size-1, fp)) > 0) {
      buffer[r] = 0;
      printf("%s", buffer);
    }

    fclose(fp);
    free(buffer);
  } else {
    // error
  }
}
  

int main(int argc, char *argv[])
{
/* Initialization. */  
  initialize();

/* Response loop. */
  while (FCGI_Accept() >= 0)   {

    char *cookie = getenv("HTTP_COOKIE");

    if (cookie == NULL) {
      // haven't seen you before
      printf("Set-Cookie: cook=%d; Max-Age=1000000\r\n", time(NULL));
      printf("Set-Cookie: cook2=%d; Max-Age=1000000\r\n", count);
    }

    char *r_url = getenv("REDIRECT_URL");
    if (strcmp(r_url, "/") == 0) {
      getFile("/var/www/html/index.html");
      continue;
    } else if (startsWith("/api/geo", r_url)) {
      getFile("/var/www/html/nz.json");
      continue;
    }
    
    printf("Content-type: text/html\r\n"
	   "Access-Control-Allow-Origin: *\r\n");
    printf("\r\n"); // header finished

    char *query = getenv("QUERY_STRING");
    
    if (startsWith("/api/customer", r_url)) {
      // customer
      customerURL(r_url, query, "/api/");
    } else if (startsWith("/api/order", r_url)) {
      // order
      orderURL(r_url, query, "/api/");
    } else {
      printf("<title>FastCGI Hello! (C, fcgi_stdio library)</title>\n"
	"<h1>FastCGI Hello! (C, fcgi_stdio library)</h1>\n"
	"Request number %d running on host <i>%s</i>\n",
	++count, getenv("SERVER_NAME"));
      
      printf("<p><pre>\n");
      char **env = environ;
      for (; *env; ++env) {
	printf("%s\n", *env);
      }
      printf("</pre>\n");
    }
    
  }
  exit(0);
}

