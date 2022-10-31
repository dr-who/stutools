FROM ubuntu:latest
MAINTAINER Stu
RUN apt update
RUN apt install -y build-essential git cmake libaio-dev libnuma-dev autoconf
ADD "https://api.github.com/repos/dr-who/stutools/commits?per_page=1" latest_commit
RUN git clone https://github.com/dr-who/stutools.git 
RUN cd stutools && cmake . && make -j && make install
ENTRYPOINT /bin/bash
