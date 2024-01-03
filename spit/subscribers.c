#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "subscribers.h"

connType *subscribersOpen() {
  connType *c = calloc(1, sizeof(connType));

  c->con = mysql_init(NULL);

  if (c->con == NULL) {
      fprintf(stderr, "%s\n", mysql_error(c->con));
      return NULL;
  }
  
  if (mysql_real_connect(c->con, "localhost", "root", "", "testdb", 0, NULL, 0) == NULL) {
      fprintf(stderr, "%s\n", mysql_error(c->con));
      mysql_close(c->con);
      return NULL;
  }
  fprintf(stderr,"*info* connected\n");
  return c;
}
  
void subscribersClose(connType *c) {
  if (c->con) {
    mysql_close(c->con);
    fprintf(stderr,"*info* closed\n");
  }
}

  
char *subscribersNewSession(connType *c) {
  if (mysql_query(con, "INSERT INTO sessions VALUES(NULL, SHA1(RANDOM_BYTES(32)), NULL)")) {
    fprintf(stderr,"*error*\n");
  }
  int id = mysql_insert_id(con);

}
