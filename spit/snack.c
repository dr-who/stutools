#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <arpa/inet.h>
#include <time.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <assert.h>

#include "transfer.h"

#include "utils.h"
#include "numList.h"

#include <glob.h>

extern volatile int keepRunning;

#include "snack.h"

stringType *listDevices(size_t *retcount) {

    stringType *ret = NULL;
    *retcount = 0;

    glob_t globbuf;

    globbuf.gl_offs = 1;
    glob("/sys/class/net/*/speed", GLOB_DOOFFS, NULL, &globbuf);

    for (size_t i = 1; i < globbuf.gl_pathc; i++) {
        FILE *fp = fopen(globbuf.gl_pathv[i], "rt");
        int speed = 0;
        char *tok1 = NULL;
        if (fp) {
            int v = fscanf(fp, "%d", &speed);
            if (v == 1) {
                //	fprintf(stderr,"%s\n", globbuf.gl_pathv[i]+15);

                tok1 = strtok(globbuf.gl_pathv[i] + 15, "/");
            }
            fclose(fp);
        }

        if (tok1 && (speed > 0)) {
            // check there is no 'owner' file
            char s2[1000];
            sprintf(s2, "/sys/class/net/%s/owner", tok1);
            FILE *fp2 = fopen(s2, "rt");
            if (fp2) { // it exists
                fclose(fp2);
            } else {
                // it doesn't exist, add it

                *retcount = (*retcount) + 1;
                ret = realloc(ret, (*retcount) * sizeof(stringType));
                ret[(*retcount) - 1].path = strdup(tok1);
                ret[(*retcount) - 1].speed = speed;
                ret[(*retcount) - 1].thistx = 0;
                ret[(*retcount) - 1].thisrx = 0;
                ret[(*retcount) - 1].thistime = 0;
            }

            //      fprintf(stderr,"speed %s %d\n", tok1, speed);
        }
    }
    globfree(&globbuf);
    return ret;
}

void getEthStats(stringType *devs, size_t num) {
    for (size_t i = 0; i < num; i++) {
        devs[i].lasttime = devs[i].thistime;
        if (0) {//(devs[i].path[0] == 'i') && (devs[i].path[1] == 'b')) {
            // glob // /sys/class/net/ibp216s0f1/device/infiniband/mlx5_3/
            glob_t globbuf;
            globbuf.gl_offs = 1;
            char s[1000];
            sprintf(s, "/sys/class/net/%s/device/infiniband/ml*", devs[i].path);
            fprintf(stdout, "--> %s --> %s\n", devs[i].path, s);
            glob(s, GLOB_DOOFFS, NULL, &globbuf);
            for (size_t i = 1; i < globbuf.gl_pathc; i++) {
                fprintf(stdout, "%s\n", globbuf.gl_pathv[i]);
            }
            globfree(&globbuf);

            // infiniband
        } else {
            char s[1000];
            devs[i].lastrx = devs[i].thisrx;
            devs[i].lasttx = devs[i].thistx;
            devs[i].thistx = -1;
            devs[i].thistxerrors = -1;
            devs[i].thisrx = -1;
            devs[i].thisrxerrors = -1;
            devs[i].speed = -1;
            devs[i].mtu = -1;
            devs[i].carrier_changes = -1;
            devs[i].numa = -1;

            devs[i].thistime = timeAsDouble();
            sprintf(s, "/sys/class/net/%s/statistics/tx_bytes", devs[i].path);
            FILE *fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%ld", &devs[i].thistx) != 1) { perror("tx"); }
                fclose(fp);
            }
            sprintf(s, "/sys/class/net/%s/statistics/tx_errors", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%ld", &devs[i].thistxerrors) != 1) { perror("tx"); }
                fclose(fp);
            }

            sprintf(s, "/sys/class/net/%s/speed", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%d", &devs[i].speed) != 1) { perror("speed"); }
                fclose(fp);
            }

            sprintf(s, "/sys/class/net/%s/statistics/rx_bytes", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%ld", &devs[i].thisrx) != 1) { perror("rx"); }
                fclose(fp);
            }
            sprintf(s, "/sys/class/net/%s/statistics/rx_errors", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%ld", &devs[i].thisrxerrors) != 1) { perror("rx"); }
                fclose(fp);
            }

            sprintf(s, "/sys/class/net/%s/carrier_changes", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%ld", &devs[i].carrier_changes) != 1) { perror("rx"); }
                fclose(fp);
            }

            sprintf(s, "/sys/class/net/%s/mtu", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%d", &devs[i].mtu) != 1) {
                    perror("mtu");
                }
                fclose(fp);
            }

            sprintf(s, "/sys/class/net/%s/device/numa_node", devs[i].path);
            fp = fopen(s, "rt");
            if (fp) {
                if (fscanf(fp, "%d", &devs[i].numa) != 1) {
                    perror("numa");
                }
                fclose(fp);
            }
        }

        //    if (devs[i].lasttime != 0) {
        double gaptime = devs[i].thistime - devs[i].lasttime;
        double txspeed = (devs[i].thistx - devs[i].lasttx) * 8.0;
        double rxspeed = (devs[i].thisrx - devs[i].lastrx) * 8.0;
        if (devs[i].speed > 0)
            fprintf(stdout,
                    "--> [%d] %s/MTU %d/speed %d Mb/s (RX %.1lf Gb/s, TX %.1lf Gb/s, errors %zd/%zd, carrier_changes %zd)\n",
                    MAX(0, devs[i].numa), devs[i].path, devs[i].mtu, devs[i].speed, TOGB(rxspeed) / gaptime,
                    TOGB(txspeed) / gaptime, devs[i].thisrxerrors, devs[i].thistxerrors, devs[i].carrier_changes);
        //    }
    } // for i
}


#include <pci/pci.h>

void dumpEthernet(void) {
    struct pci_access *pacc;
    struct pci_dev *dev;
    u8 cfg_space[4096] = {0};

    pacc = pci_alloc();  /* Get the pci_access structure */

    /* Set all options you want -- here we stick with the defaults */

    pci_init(pacc);     /* Initialize the PCI library */
    pci_scan_bus(pacc); /* We want to get the list of devices */

    for (dev = pacc->devices; dev; dev = dev->next) { /* Iterate over all devices */

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
        sprintf(str, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/current_link_speed", dev->domain, dev->bus, dev->dev,
                dev->func);
        sprintf(strw, "/sys/bus/pci/devices/%04x:%02x:%02x.%d/current_link_width", dev->domain, dev->bus, dev->dev,
                dev->func);

        printf("PCI %04x:%02x:%02x.%d NUMA=%d ",
               dev->domain, dev->bus, dev->dev,
               dev->func /*dev->vendor_id, dev->device_id,  dev->device_class, dev->irq, c, (long) dev->base_addr[0]*/,
               dev->numa_node);


        ///sys/bus/pci/devices/0000\:21\:00.0/max_link_speed

        FILE *fp = fopen(str, "rt");
        double maxlink = 0, maxwidth = 0;
        if (fp) {
            char result[100];
            int e = fscanf(fp, "%s", result);
            assert(e >= 0);
            maxlink = atof(result);
            //	printf(", max link speed = %s", result);
            fclose(fp);
        } else {
            //	perror(str);
        }

        fp = fopen(strw, "rt");
        if (fp) {
            char result[100];
            int e = fscanf(fp, "%s", result);
            assert (e >= 0);
            maxwidth = atof(result);
            fclose(fp);
        } else {
            //	perror(str);
        }


        double speed = 0;
        if (maxlink <= 5) {
            speed = maxlink * maxwidth * 8.0 / 10; // PCIe-2
        } else {
            speed = maxlink * maxwidth * 128 / 130; // PCIe-3 encoding
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


static void *receiver(void *arg) {
    threadInfoType *tc = (threadInfoType *) arg;
    while (keepRunning) {
        int serverport = tc->serverport;

        int sockfd = socket(AF_INET, SOCK_STREAM, 0);
        if (sockfd == -1) {
            perror("Can't allocate sockfd");
	    continue;
        }

        int true = 1;
        if (setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &true, sizeof(int)) == -1) {
            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
	struct timeval timeout;
	timeout.tv_sec = 5;
	timeout.tv_usec = 0;
        if (setsockopt(sockfd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)) == -1) {
            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
        if (setsockopt(sockfd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout)) == -1) {
            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
	

        struct sockaddr_in clientaddr, serveraddr;
        memset(&serveraddr, 0, sizeof(serveraddr));
        serveraddr.sin_family = AF_INET;
        serveraddr.sin_addr.s_addr = htonl(INADDR_ANY);
        serveraddr.sin_port = htons(serverport);

        if (bind(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) == -1) {
            perror("Bind Error");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }

        if (listen(sockfd, 7788 + tc->id) == -1) {
            perror("Listen Error");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }

        socklen_t addrlen = sizeof(clientaddr);
        int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
	if (connfd == -1) {
	  //	  perror("Connect Error");
	  close(sockfd);
	  continue;
	}
	    //            exit(1);
	    //        }
        char addr[INET_ADDRSTRLEN];
        inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN);
        tc->ips[tc->id] = strdup(addr);
        shutdown(sockfd, SHUT_RDWR);
        close(sockfd);

        printf("New connection from %s\n", addr);
        // reset stats
        for (int i = 0; i < tc->num; i++) {
            nlShrink(&tc->nl[i], 10);
            if (i == tc->id) {
                nlClear(&tc->nl[i]); // this one is zero
            }
        }
        //    printf("Cleared\n");

        char *buff = malloc(BUFFSIZE);
        assert(buff);
        ssize_t n;

        double lasttime = timeAsDouble();

        double sumTime = 0;
        size_t sumBytes = 0;
	
        while (keepRunning && (n = recv(connfd, buff, BUFFSIZE, 0)) > 0) {
            const double thistime = timeAsDouble();
            tc->lasttime[tc->id] = thistime;
            const double gaptime = thistime - lasttime;
            lasttime = thistime;

            sumTime += gaptime;
            sumBytes += n;

            if (sumTime > 0.01) { // every 1/100 sec then add a data point
                const double gbps = TOGB(sumBytes) * 8 / (sumTime);
                //	tc->gbps[tc->id] = gbps;
                nlAdd(&tc->nl[tc->id], gbps);

                sumTime = 0;
                sumBytes = 0;
            }

            if (n == -1) {
                perror("Receive File Error");
		continue;
		//                exit(1);
            }
        }

        close(connfd);
    }
    return NULL;
}


void *display(void *arg) {
    threadInfoType *tc = (threadInfoType *) arg;

    clock_t lastclock = clock();
    double lasttime = timeAsDouble();
    size_t count = 0;

    size_t devcount = 0;
    stringType *dev = listDevices(&devcount);


    while (keepRunning) {
        double t = 0;

        const double thistime = timeAsDouble();

        if (thistime - tc->starttime > 3600) {
            fprintf(stderr, "*warning* server running for too long, exiting\n");
            exit(1);
        }

        int clients = 0;
        for (int i = 0; i < tc->num; i++) {
            char *starsList[] = {"", "*", "**", "***", "****", "*****"};
            int stars = 0;
            assert(sizeof(starsList) / sizeof(char *) == 6);

            if (thistime - tc->lasttime[i] > 1) {
                stars = MIN(sizeof(starsList) / sizeof(char *) - 1, (size_t) (thistime - tc->lasttime[i]));;
            }

            if (nlN(&tc->nl[i]) != 0) {
                clients++;
                fprintf(stdout, "[%d - %s], mean = %.1lf Gb/s (99%% = %.1lf Gb/s), n = %zd / %zd, SD = %.4lf %s\n",
                        i + tc->serverport, tc->ips[i] ? tc->ips[i] : "", nlMean(&tc->nl[i]), nlSortedPos(&tc->nl[i], 0.99),
                        nlN(&tc->nl[i]), tc->nl[i].ever, nlSD(&tc->nl[i]), starsList[stars]);
                t += nlMean(&tc->nl[i]);
            }

            if (thistime - tc->lasttime[i] > 5) {
                if (nlN(&tc->nl[i]) != 0) {
                    for (int ii = 0; ii < tc->num; ii++) {
                        nlShrink(&tc->nl[ii], 10);
                        if (ii == i) {
                            nlClear(&tc->nl[i]);
                        }
                    }
                    //	  tc->gbps[i] = 0;
                    //	  printf("Cleared\n");
                    // reset stats
                }
            }
        }

        if (t == 0) t = NAN;

        if (count > 0) {
            /*if (!isnan(t))*/ getEthStats(dev, devcount);
            fprintf(stdout,
                    "--> time %.0lf -- total %.2lf Gb/s (%.2lf GByte/s) -- %d clients (%.2lf Gb/s/client) -- CPU %.1lf %%\n",
                    timeAsDouble(), t, t / 8.0, clients, t / clients,
                    (clock() - lastclock) * 100.0 / (timeAsDouble() - lasttime) / CLOCKS_PER_SEC);
            fflush(stdout);
        }
        count++;
        lasttime = timeAsDouble();
        lastclock = clock();
        if (keepRunning) sleep(1);
    }
    return NULL;
}

void snackServer(size_t num, const int serverport) {
    pthread_t *pt;
    threadInfoType *tc;
    double *lasttime;
    char **ips;
    numListType *nl;

    //  CALLOC(gbps, num, sizeof(double));
    CALLOC(ips, num, sizeof(char *));
    CALLOC(lasttime, num, sizeof(double));

    CALLOC(nl, num, sizeof(numListType));
    CALLOC(pt, num + 1, sizeof(pthread_t));
    CALLOC(tc, num + 1, sizeof(threadInfoType));

    for (size_t i = 0; i < num; i++) {
        nlInit(&nl[i], 1000);
    }

    for (size_t i = 0; i < num; i++) {
        tc[i].id = i;
        tc[i].num = num;
        tc[i].serverport = serverport + i;
        //    tc[i].gbps = gbps;
        tc[i].ips = ips;
        tc[i].lasttime = lasttime;
        tc[i].starttime = timeAsDouble();
        tc[i].nl = nl;
        pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
    }

    tc[num].id = num;
    tc[num].num = num;
    tc[num].serverport = serverport;
    //  tc[num].gbps = gbps;
    tc[num].ips = ips;
    tc[num].lasttime = lasttime;
    tc[num].starttime = timeAsDouble();
    tc[num].nl = nl;
    pthread_create(&(pt[num]), NULL, display, &(tc[num]));

    for (size_t i = 0; i < num + 1; i++) {
        pthread_join(pt[i], NULL);
	//	printf("thread %zd finished %d\n", i, keepRunning);
    }
    free(tc);
    free(pt);
}





///sending

void snackClient(char *ipaddress, size_t bufSizeToUse, const int serverport, const size_t threads) {
  assert(ipaddress);
  assert(bufSizeToUse);
  assert(threads >= 1);
  
#ifdef __WIN32
    WSADATA wsaData;
    WSAStartup(MAKEWORD(2, 2), &wsaData);
#endif

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        perror("Can't allocate sockfd");
        return;
    }

    struct sockaddr_in serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    long c = getDevRandomLong();
    srand(c);
    
    int tries = 0, port = 0;
    do {
        port = serverport + rand() % threads;
        serveraddr.sin_family = AF_INET;
        serveraddr.sin_port = htons(port);

        if (inet_aton(ipaddress, &serveraddr.sin_addr) < 0)
            //      if (inet_pton(AF_INET, argv[1], &serveraddr.sin_addr) < 0)
        {
            perror("IPaddress Convert Error");
            return;
        }

        if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
            //	  perror("Listening port not available");
            tries++;
            if (tries > 1000) {
                fprintf(stderr, "*error* couldn't find any ports. exiting\n");
		return;
            }
            continue;
        } else {
            break;
        }
        usleep(1);
    } while (keepRunning);

    fprintf(stderr,"*info* connected to %s on port %d, size = %zd\n", ipaddress, port, bufSizeToUse);

    char *buff = aligned_alloc(4096, bufSizeToUse);
    assert(buff);

    ssize_t n;
    clock_t lastclock = clock();
    double lasttime = time(NULL);
    size_t lastcount = 0, thiscount = 0;
    
    while (keepRunning && ((n = send(sockfd, buff, bufSizeToUse, 0)) > 0)) {
      if ((size_t)n==bufSizeToUse) {
	thiscount++;
      }
      clock_t thistime = time(NULL);
      if (thistime - lasttime > 1) {
	const clock_t thisclock = clock();
	fprintf(stdout, "*info* [port %d] CPU %.1lf %% (100%% is one core), %zd IOPS, %.1lf µs latency\n", port,
		(thisclock - lastclock) * 100.0 / (thistime - lasttime) / CLOCKS_PER_SEC, (thiscount - lastcount), 1000000.0/(thiscount - lastcount));
	lasttime = thistime;
	lastclock = thisclock;
	lastcount = thiscount;
      }
    }

    free(buff);

    close(sockfd);
    return;
}

