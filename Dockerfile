FROM almalinux
MAINTAINER Stu
RUN yum -y install cmake libaio-devel pciutils-devel numactl-devel readline-devel git gcc gcc-c++ make iproute emacs-nox
RUN yum -y install --enablerepo highavailability pcs pacemaker corosync
RUN yum -y install man man-pages nc
RUN git config --global user.email "stuart.inglis@gmail.com"
RUN git config --global user.name "Stuart Inglis"
ADD "https://api.github.com/repos/dr-who/stutools/commits?per_page=1" latest_commit
RUN git clone https://github.com/dr-who/stutools.git 
RUN cd stutools && cmake . && make -j && make install
ENTRYPOINT /bin/bash
