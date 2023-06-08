// DNS Query Program on Linux, found online and modified

//Header Files
#include <stdio.h>	//printf
#include <string.h>	//strlen
#include <stdlib.h>	//malloc
#include <sys/socket.h>	//you know what this is for
#include <arpa/inet.h>	//inet_addr , inet_ntoa , ntohs etc
#include <netinet/in.h>
#include <unistd.h>	//getpid

#include <regex.h>

#include "utils.h"
#include "dns.h"

//Types of DNS resource records :)

#define T_A 1 //Ipv4 address
#define T_NS 2 //Nameserver
#define T_CNAME 5 // canonical name
#define T_SOA 6 /* start of authority zone */
#define T_PTR 12 /* domain name pointer */
#define T_MX 15 //Mail server

//Function Prototypes
int ngethostbyname (char* , int, char*);
void ChangetoDnsNameFormat (char*,char*);
unsigned char* ReadName (unsigned char*,unsigned char*,int*);

//DNS header structure
struct DNS_HEADER
{
	unsigned short id; // identification number

	unsigned char rd :1; // recursion desired
	unsigned char tc :1; // truncated message
	unsigned char aa :1; // authoritive answer
	unsigned char opcode :4; // purpose of message
	unsigned char qr :1; // query/response flag

	unsigned char rcode :4; // response code
	unsigned char cd :1; // checking disabled
	unsigned char ad :1; // authenticated data
	unsigned char z :1; // its z! reserved
	unsigned char ra :1; // recursion available

	unsigned short q_count; // number of question entries
	unsigned short ans_count; // number of answer entries
	unsigned short auth_count; // number of authority entries
	unsigned short add_count; // number of resource entries
};

//Constant sized fields of query structure
struct QUESTION
{
	unsigned short qtype;
	unsigned short qclass;
};

//Constant sized fields of the resource record structure
#pragma pack(push, 1)
struct R_DATA
{
	unsigned short type;
	unsigned short _class;
	unsigned int ttl;
	unsigned short data_len;
};
#pragma pack(pop)

//Pointers to resource record contents
struct RES_RECORD
{
	unsigned char *name;
	struct R_DATA *resource;
	unsigned char *rdata;
};

//Structure of a Query
typedef struct
{
	unsigned char *name;
	struct QUESTION *ques;
} QUERY;

int dnsLookupA(char *hostname) {
  char name[1000];
  name[0] = 0;
  strcat(name, hostname);
  return ngethostbyname(name , T_A, "8.8.8.8");
}

int dnsLookupAServer(char *hostname, char *dns_server) {
  char name[1000];
  name[0] = 0;
  strcat(name, hostname);
  return ngethostbyname(name, T_A, dns_server);
}

/*
 * Perform a DNS query by sending a packet
 * */
int ngethostbyname(char *host , int query_type, char *dns_server)
{
	unsigned char buf[65536],*reader;
	char *qname;
	
	int i , j , stop , s;

	struct sockaddr_in a;

	struct RES_RECORD answers[20],auth[20],addit[20]; //the replies from the DNS server
	struct sockaddr_in dest;

	struct DNS_HEADER *dns = NULL;
	struct QUESTION *qinfo = NULL;

	//	printf("Resolving %s" , host);

	s = socket(AF_INET , SOCK_DGRAM , IPPROTO_UDP); //UDP packet for DNS queries

	dest.sin_family = AF_INET;
	dest.sin_port = htons(53);
	dest.sin_addr.s_addr = inet_addr(dns_server); //dns servers

	//Set the DNS structure to standard queries
	dns = (struct DNS_HEADER *)&buf;

	dns->id = (unsigned short) htons(getpid());
	dns->qr = 0; //This is a query
	dns->opcode = 0; //This is a standard query
	dns->aa = 0; //Not Authoritative
	dns->tc = 0; //This message is not truncated
	dns->rd = 1; //Recursion Desired
	dns->ra = 0; //Recursion not available! hey we dont have it (lol)
	dns->z = 0;
	dns->ad = 0;
	dns->cd = 0;
	dns->rcode = 0;
	dns->q_count = htons(1); //we have only 1 question
	dns->ans_count = 0;
	dns->auth_count = 0;
	dns->add_count = 0;

	//point to the query portion
	qname =(char*)&buf[sizeof(struct DNS_HEADER)];

	ChangetoDnsNameFormat(qname , host);
	qinfo =(struct QUESTION*)&buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname) + 1)]; //fill it

	qinfo->qtype = htons( query_type ); //type of the query , A , MX , CNAME , NS etc
	qinfo->qclass = htons(1); //its internet (lol)

	//	printf("\nSending Packet...");
	if( sendto(s,(char*)buf,sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION),0,(struct sockaddr*)&dest,sizeof(dest)) < 0)
	{
		perror("sendto failed");
	}
	//	printf("Done");
	
	//Receive the answer
	i = sizeof dest;
	//	printf("\nReceiving answer...");
	if(recvfrom (s,(char*)buf , 65536 , 0 , (struct sockaddr*)&dest , (socklen_t*)&i ) < 0)
	{
		perror("recvfrom failed");
	}
	//	printf("Done");

	dns = (struct DNS_HEADER*) buf;

	//move ahead of the dns header and the query field
	reader = &buf[sizeof(struct DNS_HEADER) + (strlen((const char*)qname)+1) + sizeof(struct QUESTION)];

	//	printf("\nThe response contains : ");
	//	printf("\n %d Questions.",ntohs(dns->q_count));
	//	printf("\n %d Answers.",ntohs(dns->ans_count));
	//	printf("\n %d Authoritative Servers.",ntohs(dns->auth_count));
	//	printf("\n %d Additional records.\n\n",ntohs(dns->add_count));

	//Start reading answers
	stop=0;

	for(i=0;i<ntohs(dns->ans_count);i++)
	{
		answers[i].name=ReadName(reader,buf,&stop);
		reader = reader + stop;

		answers[i].resource = (struct R_DATA*)(reader);
		reader = reader + sizeof(struct R_DATA);

		if(ntohs(answers[i].resource->type) == 1) //if its an ipv4 address
		{
			answers[i].rdata = (unsigned char*)malloc(ntohs(answers[i].resource->data_len));

			for(j=0 ; j<ntohs(answers[i].resource->data_len) ; j++)
			{
				answers[i].rdata[j]=reader[j];
			}

			answers[i].rdata[ntohs(answers[i].resource->data_len)] = '\0';

			reader = reader + ntohs(answers[i].resource->data_len);
		}
		else
		{
			answers[i].rdata = ReadName(reader,buf,&stop);
			reader = reader + stop;
		}
	}

	//read authorities
	for(i=0;i<ntohs(dns->auth_count);i++)
	{
		auth[i].name=ReadName(reader,buf,&stop);
		reader+=stop;

		auth[i].resource=(struct R_DATA*)(reader);
		reader+=sizeof(struct R_DATA);

		auth[i].rdata=ReadName(reader,buf,&stop);
		reader+=stop;
	}

	//read additional
	for(i=0;i<ntohs(dns->add_count);i++)
	{
		addit[i].name=ReadName(reader,buf,&stop);
		reader+=stop;

		addit[i].resource=(struct R_DATA*)(reader);
		reader+=sizeof(struct R_DATA);

		if(ntohs(addit[i].resource->type)==1)
		{
			addit[i].rdata = (unsigned char*)malloc(ntohs(addit[i].resource->data_len));
			for(j=0;j<ntohs(addit[i].resource->data_len);j++)
			addit[i].rdata[j]=reader[j];

			addit[i].rdata[ntohs(addit[i].resource->data_len)]='\0';
			reader+=ntohs(addit[i].resource->data_len);
		}
		else
		{
			addit[i].rdata=ReadName(reader,buf,&stop);
			reader+=stop;
		}
	}

	//print answers
	//	printf("\nAnswer Records : %d \n" , ntohs(dns->ans_count) );
	for(i=0 ; i < ntohs(dns->ans_count) ; i++)
	{
		printf("Name : %s ",answers[i].name);

		if( ntohs(answers[i].resource->type) == T_A) //IPv4 address
		{
			long *p;
			p=(long*)answers[i].rdata;
			a.sin_addr.s_addr=(*p); //working without ntohl
			printf("has IPv4 address : %s",inet_ntoa(a.sin_addr));
		}
		
		if(ntohs(answers[i].resource->type)==5) 
		{
			//Canonical name for an alias
			printf("has alias name : %s",answers[i].rdata);
		}

		printf("\n");
	}

	//print authorities
	if (0) {
	  printf("\nAuthoritive Records : %d \n" , ntohs(dns->auth_count) );
	  for( i=0 ; i < ntohs(dns->auth_count) ; i++)
	    {
	      
	      printf("Name : %s ",auth[i].name);
	      if(ntohs(auth[i].resource->type)==2)
		{
		  printf("has nameserver : %s",auth[i].rdata);
		}
	      printf("\n");
	    }
	}

	  
	//print additional resource records
	if (0) {
	  printf("\nAdditional Records : %d \n" , ntohs(dns->add_count) );
	  for(i=0; i < ntohs(dns->add_count) ; i++)
	    {
	      printf("Name : %s ",addit[i].name);
	      if(ntohs(addit[i].resource->type)==1)
		{
		  long *p;
		  p=(long*)addit[i].rdata;
		  a.sin_addr.s_addr=(*p);
		  printf("has IPv4 address : %s",inet_ntoa(a.sin_addr));
		}
	      printf("\n");
	    }
	}
	return ntohs(dns->ans_count);
}

/*
 * 
 * */
u_char* ReadName(unsigned char* reader,unsigned char* buffer,int* count)
{
	unsigned char *name;
	unsigned int p=0,jumped=0,offset;
	int i , j;

	*count = 1;
	name = (unsigned char*)malloc(256);

	name[0]='\0';

	//read the names in 3www6google3com format
	while(*reader!=0)
	{
		if(*reader>=192)
		{
			offset = (*reader)*256 + *(reader+1) - 49152; //49152 = 11000000 00000000 ;)
			reader = buffer + offset - 1;
			jumped = 1; //we have jumped to another location so counting wont go up!
		}
		else
		{
			name[p++]=*reader;
		}

		reader = reader+1;

		if(jumped==0)
		{
			*count = *count + 1; //if we havent jumped to another location then we can count up
		}
	}

	name[p]='\0'; //string complete
	if(jumped==1)
	{
		*count = *count + 1; //number of steps we actually moved forward in the packet
	}

	//now convert 3www6google3com0 to www.google.com
	for(i=0;i<(int)strlen((const char*)name);i++) 
	{
		p=name[i];
		for(j=0;j<(int)p;j++) 
		{
			name[i]=name[i+1];
			i=i+1;
		}
		name[i]='.';
	}
	name[i-1]='\0'; //remove the last dot
	return name;
}

/*
 * This will convert www.google.com to 3www6google3com 
 * got it :)
 * */
void ChangetoDnsNameFormat(char* dns,char* host) 
{
        size_t lock = 0;
	strcat((char*)host,".");
	
	for(size_t i = 0 ; i < strlen((char*)host) ; i++) 
	{
		if(host[i]=='.') 
		{
			*dns++ = i-lock;
			for(;lock<i;lock++) 
			{
				*dns++=host[lock];
			}
			lock++; //or lock=i+1;
		}
	}
	*dns++='\0';
}


void dnsServersInit(dnsServersType *d) {
  memset(d, 0, sizeof(dnsServersType));
}

void dnsServersRm(dnsServersType *d, size_t id) {
  if (id >= d->num) {
    fprintf(stderr,"DNS index out of range: %zd\n", id);
  } else {
    free(d->dnsServer[id]);
    d->dnsServer[id] = NULL;
  }
}
    

void dnsServersClear(dnsServersType *d) {
  for (size_t i = 0; i < d->num; i++) {
    free(d->dnsServer[i]);
  }
  free(d->dnsServer);
  dnsServersInit(d);
}

void dnsServersFree(dnsServersType *d) {
  for (size_t i = 0; i < d->num; i++) {
    free(d->dnsServer[i]);
  }
  free(d->dnsServer);
}

void dnsServersDump(dnsServersType *d) {
  for (size_t i = 0; i < d->num; i++) {
    printf("[%zd]\t%s\n", i, d->dnsServer[i]);
  }
}

void dnsServersAdd(dnsServersType *d, const char *server) {
  for (size_t i = 0; i < d->num; i++) {
    if (d->dnsServer[i] && (strcmp(d->dnsServer[i], server)==0)) {
      // found, not adding
      return;
    }
  }
  // check for empty slot
  for (size_t i = 0; i < d->num; i++) {
    if (d->dnsServer[i] == NULL) {
      // found a gap
      d->dnsServer[i] = strdup(server);
      return;
    }
  }
  // new server
  d->num++;
  d->dnsServer = realloc(d->dnsServer, (d->num) * sizeof(dnsServersType)); assert(d->dnsServer);
  d->dnsServer[d->num - 1] = strdup(server);
}

void dnsServersAddFile(dnsServersType *d, const char *filename, const char *regexstring) {
  regex_t regex;
  int reti;
  reti = regcomp(&regex, regexstring, REG_EXTENDED | REG_ICASE);
  if (reti) {
    fprintf(stderr, "*error* could not compile regex\n");
    return;
  }
  
  FILE *fp = fopen(filename, "rt");
  if (fp) {
    char *line = NULL;
    size_t len = 0;
    ssize_t nread;

    while ((nread = getline(&line, &len, fp)) != -1) {
      //      printf("Retrieved line of length %zu:\n", nread);

      reti = regexec(&regex, line, 0, NULL, 0);
      if (!reti) {
	// grab second part
	char *first = strtok(line, " \n");
	if (first) {
	  char *second = strtok(NULL, " \n");
	  if (second) {
	    dnsServersAdd(d, strdup(second));
	  }
	}
      }
    }

    free(line);
    fclose(fp);
  } else {
    perror(filename);
  }
}
	 
size_t dnsServersN(dnsServersType *d) {
  return d->num;
}
				       
