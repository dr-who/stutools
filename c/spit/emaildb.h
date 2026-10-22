#ifndef __EMAILDB_H
#define __EMAILDB_H


typedef struct {
  size_t len;
  char **addr;
} emaildbType;

emaildbType * emaildbInit(void);

void emaildbAdd(emaildbType *e, char *emailaddress);

void emaildbDump(emaildbType *e);

emaildbType * emaildbLoad(FILE *fp);

void emaildbFree(emaildbType *e);

#endif
