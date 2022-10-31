FROM ubuntu:latest
MAINTAINER Stu
RUN apt update
RUN apt install -y build-essential git cmake libaio-dev libnuma-dev autoconf
RUN git clone https://github.com/dr-who/stutools.git
RUN cd stutools && cmake . && make -j && make install
ENTRYPOINT /bin/bash
