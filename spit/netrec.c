#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <arpa/inet.h>
#include <time.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include "transfer.h"

#include "utils.h"
#include "numList.h"

int keepRunning = 1;

typedef struct {
  int id;
  int num;
  int serverport;
  double *gbps;
  double *lasttime;
  numListType nl;
  char **ips;
  double starttime;
} threadInfoType;



#include <pci/pci.h>

void dumpEthernet() {
    struct pci_access *pacc;
    struct pci_dev *dev;
    u8 cfg_space[4096] = {0};

    pacc = pci_alloc();  /* Get the pci_access structure */

    /* Set all options you want -- here we stick with the defaults */

    pci_init(pacc);     /* Initialize the PCI library */
    pci_scan_bus(pacc); /* We want to get the list of devices */

    for (dev=pacc->devices; dev; dev=dev->next)    { /* Iterate over all devices */

        if (!pci_read_block(dev, 0x0, cfg_space, 2)) {
            // handle error
	  return;
        }

	pci_fill_info(dev, -1);
	//	unsigned int c = pci_read_byte(dev, PCI_INTERRUPT_PIN);                                /* Read config register directly */
	//

	if ((dev->device_class & 0xff00) != 0x0200)
	  continue;

	char str[100], strw[100];
	sprintf(str, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/max_link_speed", dev->domain, dev->bus, dev->dev, dev->func);
	sprintf(strw, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/max_link_width", dev->domain, dev->bus, dev->dev, dev->func);
	
      printf("%04x:%02x:%02x.%d ",
	     dev->domain, dev->bus, dev->dev, dev->func /*dev->vendor_id, dev->device_id,  dev->device_class, dev->irq, c, (long) dev->base_addr[0], dev->numa_node */);


      ///sys/bus/pci/devices/0000\:21\:00.0/max_link_speed

      FILE *fp = fopen (str, "rt");
      double maxlink = 0, maxwidth = 0;
      if (fp) {
	char result[100];
	int e = fscanf(fp, "%s", result);
	assert(e>=0);
	maxlink = atof(result);
	//	printf(", max link speed = %s", result);
	fclose(fp);
      } else {
	//	perror(str);
      }

      fp = fopen (strw, "rt");
      if (fp) {
	char result[100];
	int e = fscanf(fp, "%s", result);
	assert (e>=0);
	maxwidth = atof(result);
	fclose(fp);
      } else {
	//	perror(str);
      }


      double speed = 0;
      if (maxlink <= 5) {
	speed = maxlink * maxwidth * 8.0/10; // PCIe-2
      } else {
	speed = maxlink * maxwidth * 128/130; // PCIe-3 encoding
      }
      
      printf("speed = %.1lf Gb/s (speed = %g GT/s, width = %g lanes)", speed, maxlink, maxwidth);
      /* Look up and print the full name of the device */
      char namebuf[1000];
      char *name = pci_lookup_name(pacc, namebuf, sizeof(namebuf), PCI_LOOKUP_DEVICE, dev->vendor_id, dev->device_id);
      printf(" (%s)\n", name);

    }

    pci_cleanup(pacc);
    //    return 0;
}


static void *receiver(void *arg) 
{
  threadInfoType *tc = (threadInfoType*)arg;
  //  fprintf(stderr,"%d\n", tc->serverport);
  while (keepRunning) {

    int serverport = tc->serverport;
  
    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd == -1) 
      {
        perror("Can't allocate sockfd");
        exit(1);
      }

    int true = 1;
    if (setsockopt(sockfd,SOL_SOCKET,SO_REUSEADDR,&true,sizeof(int)) == -1)
      {
	perror("Setsockopt");
	exit(1);
      }

    
    struct sockaddr_in clientaddr, serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    serveraddr.sin_family = AF_INET;
    serveraddr.sin_addr.s_addr = htonl(INADDR_ANY);
    serveraddr.sin_port = htons(serverport);

    if (bind(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) == -1) 
      {
        perror("Bind Error");
        exit(1);
      }

    if (listen(sockfd, 7788+tc->id) == -1) {
      perror("Listen Error");
      exit(1);
    }

    socklen_t addrlen = sizeof(clientaddr);
    int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
    if (connfd == -1) 
      {
        perror("Connect Error");
        exit(1);
      }
    char addr[INET_ADDRSTRLEN];
    inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN);
    tc->ips[tc->id] = strdup(addr);
    shutdown(sockfd, SHUT_RDWR);
    close(sockfd); 

    printf("New connection from %s\n", addr);
    
    char buff[MAX_LINE] = {0};
    ssize_t n;

    double lasttime = timedouble();
    ssize_t total = 0;
    
    double globaltime = 0;
    size_t globalbytes = 0;
    while ((n = recv(connfd, buff, MAX_LINE, 0)) > 0) 
      {
	double thistime = timedouble();

	tc->lasttime[tc->id] = thistime;
	if (thistime < lasttime + 5) {
	  globaltime += (thistime - lasttime);
	  globalbytes += n;
	} else {
	  globalbytes = 0;
	  globaltime = 0;
	}
	lasttime = thistime;
      
	total+=n;

	const double gbps = TOGB(globalbytes) * 8 / (globaltime);
	tc->gbps[tc->id] = gbps;
	nlAdd(&tc->nl, gbps);
      
	//	fprintf(stdout,"[%d] %zd, time %.4lf,  speed %.3lf Gb/s\n", tc->id, total, globaltime/tc->num, nlMean(&tc->nl));

	if (n == -1)
	  {
	    perror("Receive File Error");
	    exit(1);
	  }
      
	//	printf("Receive Success, NumBytes = %ld\n", total);
      }

    close(connfd);
  }
  return NULL;
}


void *display(void *arg) {
  threadInfoType *tc = (threadInfoType*)arg;

  clock_t lastclock = clock();
  double lasttime = timedouble();
  while(1) {
    double t = 0;

    if (timedouble() - tc->starttime > 3600) {
      fprintf(stderr,"*warning* server running for too long, exiting\n");
      exit(1);
    }

    int clients = 0;
    for (int i = 0; i < tc->num;i++) {
      if (timedouble() - tc->lasttime[i] > 5) {
	tc->gbps[i] = 0;
      }
      if (tc->gbps[i] != 0) {
	clients++;
	fprintf(stdout, "[%d - %s], %.1lf Gb/s\n", SERVERPORT+i, tc->ips[i] ? tc->ips[i] : "", tc->gbps[i]);
      }
      t += tc->gbps[i];
    }
    lasttime = timedouble();
    lastclock = clock();
    sleep(1);
    fprintf(stdout, "--> total %.2lf Gb/s (%.1lf GByte/s) -- %d clients -- CPU %.1lf %% (100%% is one core)\n", t, t/8.0, clients, (clock() - lastclock) *100.0 / (timedouble() - lasttime) /  CLOCKS_PER_SEC);
  }
}

void startServers(size_t num) {
  pthread_t *pt;
  threadInfoType *tc;
  double *gbps, *lasttime;
  char **ips;
  CALLOC(gbps, num, sizeof(double));
  CALLOC(ips, num, sizeof(char*));
  CALLOC(lasttime, num, sizeof(double));

  CALLOC(pt, num+1, sizeof(pthread_t));
  CALLOC(tc, num+1, sizeof(threadInfoType));

  for (size_t i = 0; i < num; i++) {
    tc[i].id = i;
    tc[i].num = num;
    tc[i].serverport = SERVERPORT + i;
    tc[i].gbps = gbps;
    tc[i].ips = ips;
    tc[i].lasttime = lasttime;
    tc[i].starttime = timedouble();
    nlInit(&tc[i].nl, 3);
    pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
  }

  tc[num].id=num;
  tc[num].num=num;
  tc[num].serverport = 0;
  tc[num].gbps = gbps;
  tc[num].ips = ips;
  tc[num].lasttime = lasttime;
  tc[num].starttime = timedouble();
  pthread_create(&(pt[num]), NULL, display, &(tc[num]));

  for (size_t i = 0; i < num+1; i++) {
    pthread_join(pt[i], NULL);
  }
  free(tc);
  free(pt);
}



int main() {

  // args

  /*  if (argc != 2) {
    fprintf(stderr,"*info* usage ./netrec\n");
    exit(1);
    }*/
  int threads = 100;
  fprintf(stderr,"*info* starting netspeed receiver -- %d ports\n", threads);
  dumpEthernet();

  if (threads < 1) threads = 1;
  // start servers
  startServers(threads);
  

  return 0;
}
