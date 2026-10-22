#include <string.h>
#include <resolv.h>
#include <unistd.h>
#include <stdlib.h>

int main (int argc, char * argv[])
{
     union {
        HEADER hdr;              
        u_char buf[NS_PACKETSZ]; 
    } response;                  
    int responseLen;             
    res_init();
    ns_msg handle;
    responseLen =res_query(argv[1],ns_c_in,ns_t_a,(u_char *)&response,sizeof(response));
    if (responseLen<0)
        exit(-1);
    if (ns_initparse(response.buf, responseLen, &handle)<0)
    {
        fprintf(stderr, "ERROR PARSING RESPONSE....");
        perror("damn");
        exit(-1);

    }
    ns_rr rr;
    int rrnum;
    ns_sect section=ns_s_an;
    for (rrnum=0;rrnum<(ns_msg_count(handle,section));rrnum++)
    {
        if (ns_parserr(&handle,ns_s_an,rrnum,&rr)<0)
        {
            fprintf(stderr, "ERROR PARSING RRs");
            exit(-1);
        }   

        if (ns_rr_type(rr)==ns_t_a)
        {
struct in_addr in;
memcpy(&in.s_addr, ns_rr_rdata(rr), sizeof(in.s_addr));
fprintf(stderr, "%s IN A %s\n", ns_rr_name(rr), inet_ntoa(in));
        }
    }
    return 0;   
}

