
#include <malloc.h>
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <semaphore.h>

#include "cluster.h"
#include "simpmail.h"
#include "keyvalue.h"
#include "utils.h"


// only change the times when an element changes value


// a cluster all has the same port
clusterType * clusterInit(const size_t port) {
  fprintf(stderr,"clusterInit on port %zd\n", port);
  clusterType *p = calloc(1, sizeof(clusterType)); assert(p);
  p->port = port;
  if (sem_init(&p->sem, 1, 1)) {
    fprintf(stderr,"sem_init()!\n");
    exit(1);
  }

  int fd = simpmailConnect("127.0.0.1");
  if (fd > 0) {
    p->localsmtp = 1;
  }
  close(fd);

  return p;
}

// returns -1 for can't find it
int clusterFindNode(clusterType *c, const char *node) {
  sem_wait(&c->sem);
  int ret = -1;
  assert(c);
  if (c) {
    for (int i = 0; i < c->id; i++) {
      if (c->node) {
	if (c->node[i]) {
	  if (strcasecmp(c->node[i]->name, node)==0) {
	    ret = i;
	    break;
	  }
	}
      }
    }
  }
  sem_post(&c->sem);
  return ret;
}


double clusterCreated(clusterType *c) {
  double oldest = timeAsDouble();
  for (int cc = 0; cc < c->id; cc++) {
    if (c->node[cc]->created < oldest) {
      oldest = c->node[cc]->created;
    }
  }
  return oldest;
}


void clusterSetAlertEmail(clusterType *c, char *toemail, char *fromemail, char *fromname, char *subject) {
  if (toemail) {
    fprintf(stderr,"*info* setting alert toemail to '%s'\n", toemail);
    fprintf(stderr,"*info* setting alert fromemail to '%s'\n", fromemail);
    fprintf(stderr,"*info* setting alert fromname to '%s'\n", fromname);
    fprintf(stderr,"*info* setting alert subject to '%s'\n", subject);
    c->alertToEmail = strdup(toemail);
    c->alertFromEmail = strdup(fromemail);
    c->alertFromName = strdup(fromname);
    c->alertSubject = strdup(subject);
  } else {
    c->alertToEmail = NULL;
  }
}

int clusterAddNode(clusterType *c, const char *node, const double createdtime) {
  assert(c);
  sem_wait(&c->sem);
  const int index = c->id;
  for (int i = 0; i < index; i++) {
    if (strcasecmp(c->node[i]->name, node)==0) {
      fprintf(stderr,"*warning* node already there, ignoring\n");
      goto end;
    }
  }

  c->id++;
  c->node = realloc(c->node, c->id * sizeof(clusterNodeType *));
  c->node[index] = calloc(sizeof(clusterNodeType), 1); // create cluster node
  memset(c->node[index], 0, sizeof(clusterNodeType));
  c->node[index]->name = strdup(node);
  c->node[index]->created = createdtime;
  c->node[index]->discovered = timeAsDouble();
  c->node[index]->changed = c->node[index]->created;

  c->node[index]->info = keyvalueInit(); // 

  c->latestchange = c->node[index]->created;

 end:
  sem_post(&c->sem);
  return index;
}

char * clusterGoodBad(clusterType *c, size_t *nodesGood, size_t *nodesBad) { 
  *nodesGood = 0;
  *nodesBad = 0;
  char *retlist = calloc(100000,1);

  for (int cc = 0; cc < c->id; cc++) {
    if (timeAsDouble() - c->node[cc]->seen <= 11) {
      (*nodesGood)++;
      if (c->node[cc]->badsince > 0) {
	// in a run, add to it
	c->node[cc]->nodedowntime += (timeAsDouble() - c->node[cc]->badsince);
	c->node[cc]->badsince = 0;
      }
    } else {
      if (c->node[cc]->badsince == 0) {
	// start of a bad run
	if (c->node[cc]->badsince == 0) {
	  // if first time
	  c->node[cc]->badsince = c->node[cc]->seen;
	}
      }
      
      if (*nodesBad != 0) {
	strcat(retlist, " ");
      }
      (*nodesBad)++;
      char *nodename = keyvalueGetString(c->node[cc]->info, "nodename");
      strcat(retlist, nodename);
      free(nodename);
    }
  }
  char *ret = strdup(retlist);
  free(retlist);
  return ret;
}

void clusterSendAlertEmail(clusterType *c) {
  
  if (c->localsmtp) {
    char body[1000], subject[100];
    size_t nodesGood = 0, nodesBad = 0;
    char *badlist = clusterGoodBad(c, &nodesGood, &nodesBad);
    
    int fd = simpmailConnect("127.0.0.1");
    if (fd > 0) {

      if (nodesBad > nodesGood) {
	if (c->downSince == 0) {
	  c->downSince = timeAsDouble();
	}
      } else {
	// good >= bad
	if (c->downSince) {
	  c->downTime += (timeAsDouble() - c->downSince);
	}
      }
      
      if (nodesBad) {
	c->alertLastTime = timeAsDouble();
	c->alertCount++;
      } else {
	// nodesBad is now 0
	if (c->alertLastTime) {// if in alert
	  c->downTime += (timeAsDouble() - c->alertLastTime);
	} else {
	  // new node ?
	}
      }
      const int thisClusterSize = c->id;
      double totalnodedowntime = 0, totalnodetime = 0;
      int allNodesAgreeSize = 1;
      for (int cc = 0; cc < c->id; cc++) {
	totalnodedowntime += c->node[cc]->nodedowntime;
	totalnodetime += (timeAsDouble() - c->node[cc]->created);
	
	int cansee= keyvalueGetLong(c->node[cc]->info, "cluster");
	if (cansee != thisClusterSize) {
	  fprintf(stderr,"*warning* we see %d nodes, node %s sees %d\n", thisClusterSize, c->node[cc]->name, cansee);
	  allNodesAgreeSize = 0;
	}
      }
	  
	  

      double clusterage = timeAsDouble() - clusterCreated(c);
      
      sprintf(body, "Event: %zd\n"
	      "AllNodesAgreeSize: %s\n"
	      "ClusterPort: %zd\n"
	      "ClusterNodes: %d\n"
	      "BadNodes: %zd (%s)\n"
	      "NodeDownTime: %.1lf secs (%.1lf %%)\n"
	      "ClusterDownTime: %.1lf secs (%.1lf %%)\n"
	      "ClusterUpTime: %.1lf secs (%.1lf %%)\n",
	      c->alertCount,
	      allNodesAgreeSize ? "Yes" : "No",
	      c->port,
	      c->id,
	      nodesBad, badlist,
	      totalnodedowntime, (totalnodedowntime * 100.0 / totalnodetime),
	      c->downTime, (c->downTime) *100.0 / clusterage,
	      clusterage, (clusterage - c->downTime) *100.0 / clusterage);
      
      sprintf(subject, "[%.0lf] Event#%zd %s %s", c->alertLastTime, c->alertCount, nodesBad ? "DOWN" : "UP", c->alertSubject);
      
      simpmailSend(fd, 1, c->alertFromEmail, c->alertFromName, c->alertToEmail, NULL, NULL, subject, NULL, body);
      simpmailClose(fd);
      fprintf(stderr,"*info* ALERT [%zd] email: %s", c->alertCount, body);

      if (c->alertLastTime) {
	if (nodesBad == 0) {
	  c->alertLastTime = 0;
	}
      }
		  
    }
    free(badlist);
  } else {
    fprintf(stderr,"*ALERT* no email setup\n");
  }
}

char *clusterDumpJSONString(clusterType *c) {
  if (c==NULL) {
    return NULL;
  }

  sem_wait(&c->sem);
  
  char *buf = calloc(1,1000000);
  char *ret = buf;

  // sort
  /*  if (c) 
    for (int i = 0; i < c->id-1; i++) {
      for (int j = i+1; j < c->id; j++) {
      if (strcmp(c->node[i]->ipaddress, c->node[j]->ipaddress) > 0) {
	// swap
	clusterNodeType t = *c->node[i];
	*c->node[i] = *c->node[j];
	*c->node[j] = t;
      }
    }
    }*/

  const double now = timeAsDouble();
  if (c) {
    char *tmp;
    
    buf += sprintf(buf, "{ \"clusterport\": %zd,\n", c->port);
    buf += sprintf(buf, "  \"latestchange\": %lf,\n", c->latestchange);
    buf += sprintf(buf, "  \"latestchangesecondsago\": %lf,\n", now - c->latestchange);
    buf += sprintf(buf, "  \"localSMTP\": %d,\n", c->localsmtp);
    buf += sprintf(buf, "  \"alertEvents\": %zd,\n", c->alertCount);
    buf += sprintf(buf, "  \"alertToEmail\": \"%s\",\n", c->alertToEmail ? c->alertToEmail : "");
    buf += sprintf(buf, "  \"alertFromEmail\": \"%s\",\n", c->alertFromEmail ? c->alertFromEmail : "");
    buf += sprintf(buf, "  \"alertFromName\": \"%s\",\n", c->alertFromName ? c->alertFromName : "");
    buf += sprintf(buf, "  \"alertSubject\": \"%s\",\n", c->alertSubject ? c->alertSubject : "");
    buf += sprintf(buf, "  \"nodesCount\": %d,\n", c->id);
    size_t nodesGood = 0, nodesBad = 0;
    char *badlist = clusterGoodBad(c, &nodesGood, &nodesBad);
    buf += sprintf(buf, "  \"nodesGood\": %zd,\n", nodesGood);
    buf += sprintf(buf, "  \"nodesBad\": %zd,\n", nodesBad);
    buf += sprintf(buf, "  \"nodesBadList\": \"%s\",\n", badlist);
    free(badlist);
    int sum = 0, sum4 = 0, sum8 =0, sum16 = 0, sum32 = 0, sum64 = 0;
    long ramgb = 0;
    for (int i = 0; i < c->id; i++) {
      ramgb = keyvalueGetLong(c->node[i]->info, "RAMGB");
      if (ramgb< 4) {
	sum++;
      } else if (ramgb < 8) {
	sum4++;
      } else if (ramgb< 16) {
	sum8++;
      } else if (ramgb < 32) {
	sum16++;
      } else if (ramgb < 64) {
	sum32++;
      } else {
	sum64++;
      }
	
	
    }
    buf += sprintf(buf, "  \"nodeRAM0to3\": %d,\n", sum);
    buf += sprintf(buf, "  \"nodeRAM4to7\": %d,\n", sum4);
    buf += sprintf(buf, "  \"nodeRAM8to15\": %d,\n", sum8);
    buf += sprintf(buf, "  \"nodeRAM16to31\": %d,\n", sum16);
    buf += sprintf(buf, "  \"nodeRAM32to63\": %d,\n", sum32);
    buf += sprintf(buf, "  \"nodeRAM64up\": %d,\n", sum64);
    buf += sprintf(buf, "  \"nodes\": [\n");
    for (int i = 0; i < c->id; i++) {
      buf += sprintf(buf, "    {\n");
      buf += sprintf(buf, "       \"node\": \"%s\",\n", c->node[i]->name);
      buf += sprintf(buf, "       \"nodename\": \"%s\",\n", tmp=keyvalueGetString(c->node[i]->info, "nodename")); free(tmp);
      buf += sprintf(buf, "       \"nodehw\": \"%s\",\n", tmp=keyvalueGetString(c->node[i]->info, "nodehw")); free(tmp);
      buf += sprintf(buf, "       \"nodedowntime\": %lf,\n", c->node[i]->nodedowntime);
      buf += sprintf(buf, "       \"badsince\": %lf,\n", c->node[i]->badsince);
      buf += sprintf(buf, "       \"nodeOS\": \"%s\",\n", tmp = keyvalueGetString(c->node[i]->info, "nodeOS")); free(tmp);
      buf += sprintf(buf, "       \"boardname\": \"%s\",\n", tmp = keyvalueGetString(c->node[i]->info, "boardname")); free(tmp);
      buf += sprintf(buf, "       \"biosdate\": \"%s\",\n", tmp = keyvalueGetString(c->node[i]->info, "biosdate")); free(tmp);
      buf += sprintf(buf, "       \"lastseen\": %.0lf,\n", now - c->node[i]->seen);
      buf += sprintf(buf, "       \"created\": %lf,\n", c->node[i]->created);
      buf += sprintf(buf, "       \"age\": %lf,\n", now - c->node[i]->created);
      double delay = c->node[i]->discovered - c->node[i]->created;
      if (delay < 0) delay = 0;
      buf += sprintf(buf, "       \"discoveredafter\": %lf,\n", delay);
      buf += sprintf(buf, "       \"changed\": %lf,\n", c->node[i]->changed);
      buf += sprintf(buf, "       \"sincechanged\": %lf,\n", now - c->node[i]->changed);
      buf += sprintf(buf, "       \"ipaddress\": \"%s\",\n", tmp=keyvalueGetString(c->node[i]->info, "ip"));
      free(tmp);
      buf += sprintf(buf, "       \"RAMGB\": %ld,\n", keyvalueGetLong(c->node[i]->info, "RAMGB"));
      buf += sprintf(buf, "       \"Cores\": %ld,\n", keyvalueGetLong(c->node[i]->info, "Cores"));
      buf += sprintf(buf, "       \"HDDcount\": %ld,\n", keyvalueGetLong(c->node[i]->info, "HDDcount"));
      buf += sprintf(buf, "       \"HDDsizeGB\": %ld,\n", keyvalueGetLong(c->node[i]->info, "HDDSizeGB"));
      buf += sprintf(buf, "       \"SSDcount\": %ld,\n", keyvalueGetLong(c->node[i]->info, "SSDcount"));
      buf += sprintf(buf, "       \"SSDsizeGB\": %ld\n", keyvalueGetLong(c->node[i]->info, "SSDSizeGB"));
      
      buf += sprintf(buf, "    }");
      if (i < c->id-1) buf += sprintf(buf, ",");
      buf += sprintf(buf, "\n");
    }
    buf += sprintf(buf, "  ]\n");
    buf += sprintf(buf, "}\n");
  }

  char *ret2 = strdup(ret);
  free(ret);

  sem_post(&c->sem);
  
  return ret2;
}

      
void clusterDumpJSON(FILE *fp, clusterType *c) {
  char * s = clusterDumpJSONString(c);
  fprintf(fp, "%s", s);
  free(s);
}


void clusterUpdateSeen(clusterType *c, const size_t nodeid) {
  c->node[nodeid]->seen = timeAsDouble();
}

void clusterChanged(clusterType *c, const size_t nodeid) {
  //  fprintf(stderr,"cluster changed %zd\n", nodeid);
  c->node[nodeid]->changed = timeAsDouble();
  c->latestchange = c->node[nodeid]->changed;
}

void clusterFree(clusterType **cin) {
  clusterType *c = *cin;
  const int index = c->id;
  for (int i = 0; i <index; i++) {
    free(c->node[i]->name);
    keyvalueFree(c->node[i]->info);
    c->node[i]->info = NULL;
    free(c->node[i]);
    c->node[i] = NULL;
  }
  free(c->node); c->node = NULL;
  free(c);
  *cin = NULL;
}
   

size_t clusterNameToPort(const char *name) {
  size_t sum = 0;
  if (name) {
    for (size_t i = 0; i < strlen(name); i++) {
      int val = *(name + i);
      size_t calc = i+1;
      calc = (1000-i) * val;
      sum += calc;
    }
  }
  //  fprintf(stderr,"sum: %zd\n", sum);
  if (sum) {
    sum -= (341650 + 1024);
  } else if (sum == 0) {
    sum = 6000;
  }
  return 1024 + sum % 30000;
}


size_t clusterDefaultPort(void) {
  int port = 0;
  
  if (getenv("SAT_NAME")) {
    port = clusterNameToPort(getenv("SAT_NAME"));
    fprintf(stderr,"*info* SAT_NAME is %s and used to generate port %d\n", getenv("SAT_NAME"), port);
  }
  if (port == 0) {
    if (getenv("SAT_PORT")) {
      port = atoi(getenv("SAT_PORT"));
      fprintf(stderr,"*info* SAT_PORT is %s and used to generate port %d\n", getenv("SAT_PORT"), port);
    }
  }
  if (port == 0) {
    port = clusterNameToPort(getlogin());
    fprintf(stderr,"*info* username '%s' used to generate port %d\n", getlogin(), port);
  }
  return port;
}
