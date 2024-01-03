#ifndef _SUBSCRIBERS_H
#define _SUBSCRIBERS_H

#include <mysql.h>

typedef struct {
  MYSQL *con;
} connType;

connType * subscribersOpen(); // open the DB, return a connection
void subscribersClose(connType *c); // close the DB. 


size_t subscribersLogin(int id, int TOTP); // create a sessionid
int subscribersLogout(int id);

int subscribersCreate(const char *name); // creates a new entry, for sessions and key
int subscribersCreateKey(int id); // makes a new private key
int subscribersDelete(int id); // permanently remove information about a subscriber

int subscribersSetEmail(int id, const char *email);
void subscribersEmailWelcome(int id); // emails the link to the QR code

int subscribersParseURL(char *url, size_t session);


char *subscribersNewSession(connType *c); // returns session

int subscribersPrevSession(const char *session);
void subscribersRemoveSession(const char *session);
void subscribersRemoveAllSessions(int id);

#endif
