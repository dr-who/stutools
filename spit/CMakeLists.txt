cmake_minimum_required (VERSION 3.0)
project (stutools)

get_directory_property(hasParent PARENT_DIRECTORY)
if(NOT hasParent)
  include( ${CMAKE_CURRENT_SOURCE_DIR}/../utils/Version.cmake)
endif()

execute_process(COMMAND git describe --tags
    OUTPUT_VARIABLE GIT_REV
    ERROR_QUIET
    OUTPUT_STRIP_TRAILING_WHITESPACE)

# This is used if there is no tag or the working dir is not a git repo.
if("${GIT_REV}" STREQUAL "")
    set(GIT_REV "1.1-unofficial")
endif()

set(VERSION "const char* SPIT_VERSION=\"${GIT_REV}\";")

if(NOT EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/spit-version.h)
    file(WRITE ${CMAKE_CURRENT_SOURCE_DIR}/spit-version.h "${VERSION}")
endif()

include (CTest)

#enable_testing()

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wextra -Wall -pedantic --std=gnu11 -O2" )
#SET (CMAKE_C_COMPILER             "/usr/bin/clang")


add_library(spitlib STATIC positions.c devices.c utils.c diskStats.c logSpeed.c aioRequests.c jobType.c histogram.c spitfuzz.c blockVerify.c lengths.c workQueue.c list.c latency.c numList.c procDiskStats.c mapVoid.c)

add_executable(spit spit.c spit-version.h)
target_link_libraries(spit spitlib m aio pthread numa)

add_executable(snack snackmain.c snack.c snack.h transfer.h)
target_link_libraries(snack spitlib m aio pthread numa pci)

add_executable(numListTest numListTest.c)
target_link_libraries(numListTest spitlib m aio pthread numa)

add_executable(pair pair.c )
target_link_libraries(pair spitlib m )

add_executable(raidsimulation raidsimulationmain.c failureType.c deviceProbs.c numList.c )
target_link_libraries(raidsimulation m )

add_executable(stush stush.c snack.h snack.c failureType.c deviceProbs.c pciUtils.c)
target_link_libraries(stush spitlib m pthread aio numa readline pci)


add_executable(stat stat.c )
target_link_libraries(stat spitlib m )

add_executable(pvalue pvalue.c )
target_link_libraries(pvalue spitlib m )

add_executable(positionTest positionTest.c)
target_link_libraries(positionTest spitlib m aio pthread numa )

add_executable(diskperf diskperf.c)
target_link_libraries(diskperf spitlib m aio pthread numa)

add_executable(diskpoll diskpoll.c)
target_link_libraries(diskpoll spitlib m aio pthread numa)

add_executable(diskserial diskserial.c)
target_link_libraries(diskserial spitlib m aio pthread numa)

add_executable(pmemwreck pmemwreck.c)
target_link_libraries(pmemwreck spitlib m aio pthread numa)

add_executable(xorrupt xorrupt.c)
target_link_libraries(xorrupt spitlib m aio pthread numa)

add_executable(entropy entropy.c)
target_link_libraries(entropy m)

add_executable(randdd randdd.c)
target_link_libraries(randdd spitlib m aio pthread numa)

add_executable(trim trim.c)
target_link_libraries(trim spitlib m aio pthread numa)

add_executable(median2d median2d.c)
target_link_libraries(median2d spitlib m)

add_executable(fsfiller fsfiller.c)
target_link_libraries(fsfiller spitlib m aio pthread numa)

add_executable(spitchecker verify.c)
target_link_libraries(spitchecker spitlib m aio pthread numa)

add_executable(spitchecker2 bv2.c)
target_link_libraries(spitchecker2 spitlib m aio pthread numa )

add_executable(positionLoader positionLoader.c)
target_link_libraries(positionLoader spitlib m aio pthread numa)

add_executable(raidcorrupt raidcorrupt.c)
target_link_libraries(raidcorrupt spitlib m aio pthread numa)

add_executable(devcontents devcontents.c sha-256.c)
target_link_libraries(devcontents spitlib m aio pthread numa)

add_executable(bdinfo bdinfo.c)
target_link_libraries(bdinfo spitlib m aio pthread numa)

add_executable(dtest directories.c)
target_link_libraries(dtest spitlib m aio pthread numa)

add_executable(hist hist.c)
target_link_libraries(hist spitlib m numa)

add_executable(montehist montehist.c)
target_link_libraries(montehist spitlib m numa pthread numa aio)

add_executable(genpw genpw.c)
target_link_libraries(genpw spitlib m numa pthread numa aio)

add_executable(genpositions genpositions.c)
target_link_libraries(genpositions spitlib m aio pthread numa)


#add_executable(testpositions testpositions.c)
#target_link_libraries(testpositions spitlib m pthread aio)

add_executable(testDiskStats testDiskStats.c)
target_link_libraries(testDiskStats spitlib m numa pthread)


INSTALL(FILES ${CMAKE_CURRENT_BINARY_DIR}/spit.1 DESTINATION ${CMAKE_INSTALL_PREFIX}/man/man1)
INSTALL(FILES ${CMAKE_CURRENT_BINARY_DIR}/spitchecker.1 DESTINATION ${CMAKE_INSTALL_PREFIX}/man/man1)

install(TARGETS spit fsfiller spitchecker raidcorrupt bdinfo dtest hist snack DESTINATION bin)

install(TARGETS stush PERMISSIONS SETUID WORLD_EXECUTE DESTINATION bin )

# first check the position validation/collision collapsing is working
#add_test(tds testDiskStats)


add_test(testnumList numListTest)
add_test(testPositionTest positionTest)
add_test(testspitHelp  spit -h)

add_test(testspitHelpnoargs  spit)
set_tests_properties(testspitHelpnoargs PROPERTIES WILL_FAIL TRUE )

add_test(testspitfail1  spit -c r)
set_tests_properties(testspitfail1 PROPERTIES WILL_FAIL TRUE )

add_test(testspitfail2  spit -f /dev/wowwww)
set_tests_properties(testspitfail2 PROPERTIES WILL_FAIL TRUE )

add_test(testspitfail3  spit -r)
set_tests_properties(testspitfail3 PROPERTIES WILL_FAIL TRUE )

add_test(testspitwk512b  spit -f wow -G 0.5GiB -c ws1k0.5 -v -V -t 1)

add_test(testspita10 spit -f wow -G 0.5GiB -c ws1a10zM1 -v -t 5 )

add_test(testspitkPa0fail spit -f wow -G 1GiB -c rM1s0P1X1C -K 1,0)
set_tests_properties(testspitkPa0fail PROPERTIES WILL_FAIL TRUE)

# the -K option enables us to assert the readIO and writeIO

add_test(testspitwaitforX1 spit -f wow -G 1GiB -c rk128X1s1W0.5:1 -K8192,0)
add_test(testspitwaitforX2 spit -f wow -G 1GiB -c rk128X2s1W0.5:1 -K16384,0)

add_test(testspitcc spit -f wow -G 1GiB -c cM1zx1 -K 512,512)

add_test(testspitrs1X1rP10 spit -f wow -G 1GiB -c rs1M1P10X1 -K 10,0)
add_test(testspitrs1X1rP50 spit -f wow -G 1GiB -c rs1M1P10X5 -K 50,0)
add_test(testspitrs1X1wP10 spit -f wow -G 1GiB -c ws1M1P10X1 -K 0,10)
add_test(testspitrs1x1 spit -f wow -G 1GiB -c rs1M1x1 -K 1024,0)
add_test(testspitrs1x2 spit -f wow -G 1GiB -c rs1M1x2 -K 2048,0)
add_test(testspitrs1x1n spit -f wow -G 1GiB -c rs1M1x1n -K 1024,0)
add_test(testspitrs1x2n spit -f wow -G 1GiB -c rs1M1x2n -K 2048,0)
add_test(testspitrs1x2N spit -f wow -G 1GiB -c rs1M1x2N -K 2048,0)


add_test(testspitrs1nx1 spit -f wow -G 1GiB -c rs1M1nx1 -K 1024,0)
add_test(testspitws1nx2 spit -f wow -G 1GiB -c ws1M1nx2 -K 0,2048)
add_test(testspitrs1nx2 spit -f wow -G 1GiB -c rs1M1nx2 -K 2048,0)

add_test(testspitk1a spit -f wow -G 1GiB -c rM2s0x1 -K 512,0 )
add_test(testspitk1b spit -f wow -G 1GiB -c rM1s0x1 -K 1024,0 )
add_test(testspitk1c spit -f wow -G 1GiB -c rk4s0x1 -K 262144,0 )
add_test(testspitk1d spit -f wow -G 1GiB -c wM2s0x1 -K 0,512 )
add_test(testspitk1e spit -f wow -G 1GiB -c wM1s0x1 -K 0,1024 )
add_test(testspitk1f spit -f wow -G 1GiB -c wk4s0x1 -K 0,262144 )
add_test(testspitk1g spit -f wow -G 1GiB -c rwM2s0x1 -K 250,262 -R1 )

add_test(testspitk1h spit -f wow -G 1GiB -c rM2s0x2 -K 1024,0 )
add_test(testspitk1i spit -f wow -G 1GiB -c rM2s0X1 -K 167,0 -L0.00001 )
add_test(testspitk1j spit -f wow -G 1GiB -c rM2s0X2 -K 334,0 -L0.00001 )
add_test(testspitk1k spit -f wow -G 1GiB -c rM1s0x1 -K 1024,0 -L0.00001 )

add_test(testspitk1l spit -f wow -G 1GiB -c rM1s0x1j2 -K 1024,0)
add_test(testspitk1m spit -f wow -G 1GiB -c rM1s0x1j8 -K 1024,0)
add_test(testspitk1n spit -f wow -G 1GiB -c rM1s0x2j8 -K 2048,0)
add_test(testspitk1o spit -f wow -G 1GiB -c rM1s0X1 -K 1024,0)
add_test(testspitk1pj4 spit -f wow -G 1GiB -c rM1s0X1j4 -K 1024,0 -R1)
add_test(testspitk1pJ4 spit -f wow -G 1GiB -c rM1s0X1J4 -K 4096,0 -R1)
add_test(testspitk1q spit -f wow -G 1GiB -c rM1s0X1j8 -K 1024,0 -R1)
add_test(testspitk1r spit -f wow -G 1GiB -c rM1s0X2j8 -K 2048,0 -R1)


add_test(testspitkPa1 spit -f wow -G 1GiB -c rM1s0P1X10 -K 10,0)
add_test(testspitkPa10ve spit -f wow -G 1GiB -c rM1s0P-10X1 -K 10,0)
add_test(testspitkPa100ve spit -f wow -G 1GiB -c rM1s0P-100X1 -K 100,0)
add_test(testspitkPa spit -f wow -G 1GiB -c rM1s0P100X1 -K 100,0)
add_test(testspitkPb spit -f wow -G 1GiB -c rM1s0P100X2 -K 200,0)
add_test(testspitkPc spit -f wow -G 1GiB -c rM1s0P100x1 -K 1024,0)
add_test(testspitkPcx2n spit -f wow -G 1GiB -c rM1s0P1000x2n -K 2048,0)
add_test(testspitkPcx2N spit -f wow -G 1GiB -c rM1s0P1000x2N -K 2048,0)
add_test(testspitkPcX2n spit -f wow -G 1GiB -c rM1s0P1000X2n -K 2000,0)
add_test(testspitkPcX2N spit -f wow -G 1GiB -c rM1s0P1000X2N -K 2000,0)
add_test(testspitkPcN spit -f wow -G 1GiB -c rM1s0P1000X2N -K 2000,0)
add_test(testspitkPd spit -f wow -G 1GiB -c rM1s0P-100x1 -K 1024,0)
add_test(testspitkPdw spit -f wow -G 1GiB -c wM1s0P-100x1 -K 0,1024 -R1 -v)
add_test(testspitkPdwj8 spit -f wow -G 1GiB -c wM1s0P-100x1j8 -K 0,1024 -R1 -v)

add_test(testspitkY1 spit -f wow -G 1GiB -c rM1s0P-100x1Y1 -K 1,0)
add_test(testspitkY2 spit -f wow -G 1GiB -c rM1s0P100x1Y2 -K 2,0)
add_test(testspitkY3 spit -f wow -G 1GiB -c rM1s0x1Y3 -K 3,0)
add_test(testspitkY4 spit -f wow -G 1GiB -c rM1s0x3Y4 -K 4,0)
add_test(testspitkY5 spit -f wow -G 1GiB -c rM1s0x2Y5 -K 5,0)

add_test(testspitX1N spit -f wow -G 1GiB -c rM1s0X1N -K 1024,0)
add_test(testspitX2N spit -f wow -G 1GiB -c rM1s0X2N -K 2048,0)

add_test(testspitkPplus spit -f wow -G 1GiB -c rM1s1P100X1 -K 100,0)
add_test(testspitkPplus2 spit -f wow -G 1GiB -c rM1s1P+100X1 -K 100,0)

add_test(testspitkPdot spit -f wow -G 1GiB -c rM1s1P.100X1 -K 100,0)
add_test(testspitkPdot2 spit -f wow -G 1GiB -c rM1s1P.200X1z -K 200,0)

add_test(testspitkPq11 spit -f wow -G 1GiB -c rs0k64P10q1x1 -K 16384,0)
add_test(testspitkPq12 spit -f wow -G 1GiB -c rs0k64P10q9x1 -K 16384,0)
add_test(testspitkPq13 spit -f wow -G 1GiB -c rs0k64P10q10x1 -K 16384,0)

add_test(testspitrange1 spit -f wow -G 1GiB -c rM1024q1j1x1 -v  )
add_test(testspitrange2 spit -f wow -G 1GiB -c rM1024q1j2x1o -v  )
set_tests_properties(testspitrange2 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange3 spit -f wow -G 1GiB -c rM1025q1x1 -v  )
set_tests_properties(testspitrange3 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange4 spit -f wow -G 0.5GiB -c rM513q1x1 -v  )
set_tests_properties(testspitrange4 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange5 spit -f wow -G 0.5GiB -c rM512q1x1 -v  )
add_test(testspitrange6 spit -f wow -G 0.5GiB -c rM256q1x1j2 -v  )

add_test(testspitrange7 spit -f wow -G 0.5GiB -c rM257q1x1j2 -v  )
set_tests_properties(testspitrange7 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange8 spit -f wow -G 1GiB -c rM4096 -v  )
set_tests_properties(testspitrange8 PROPERTIES WILL_FAIL TRUE)

add_test(testspita10x3n spit -f wow -G 1GiB -c ws1zM1x3n -v -t 5 )

add_test(testspitPlotsa spit -f wow -G 1GiB -c ws0P10000M64j16 -v -t 5 )
add_test(testspitPlotsb spit -f wow -G 1GiB -c ws0M64j16P100000 -v -t 5 )
add_test(testspitPlotsc spit -f wow -G 1GiB -c ws0k8j16P10000 -v -t 5 )
add_test(testspitPlotsb spit -f wow -G 1GiB -c ws0j16P10000k8 -v -t 5 )

add_test(testspitpf10 spit -p f10 -f wow -G 1GiB -c w -v -t 5)
add_test(testspitwm1 spit -f wow -G 1GiB -c wm1s1 -v -t 5)
add_test(testspitwm2 spit -f wow -G 1GiB -c wm2s1 -v -t 5)
add_test(testspitwm4 spit -f wow -G 1GiB -c wm4s1 -v -t 5)
add_test(testspitwm4q2 spit -f wow -G 1GiB -c wm4s1q2 -v -t 5)
add_test(testspitwm4K0 spit -f wow -G 1GiB -c ws1zk4K0x1 -v -t 5)
add_test(testspitwm4K1 spit -f wow -G 1GiB -c ws1zk4K1x1 -v -t 5)
add_test(testspitwm4K10 spit -f wow -G 1GiB -c ws1zk4K10x1 -v -t 5)
add_test(testspitwm4q6 spit -f wow -G 1GiB -c wm4s1q6 -v -t 5)
add_test(testspitwm_1 spit -f wow -G 1GiB -c wm-1s1 -v -t 5)

add_test(testspitF1noj spit -F wowf -G 1GiB -c ws1 -v -t 5)
add_test(testspitF2j1 spit -F wowf -G 1GiB -c ws1j1 -v -t 5)
add_test(testspitF3j2 spit -F wowf -G 0.5 -c ws1j2 -v -t 5)
add_test(testspitF4j4 spit -F wowf -G 0.25 -c ws1j2 -v -t 5)

add_test(testspitFS100 spit -f wow -G 1GiB -c ws1j2S100 -v -t 5)
add_test(testspitFS100_10 spit -f wow -G 1GiB -c ws1j2S100-10 -v -t 5)

add_test(testspj2rangewmUj1 spit -f wow -G 1GiB -c wUs0k4-256 -v -t 5 )
add_test(testspj2rangewmP1000 spit -f wow -G 1GiB -c ws0k8-16P1000 -v -t 5 )
add_test(testspj2rangewmP1000j17 spit -f wow -G 1GiB -c ws0k8-16P1000j17u -v -t 5 )
add_test(testspj2rangewm spit -f wow -G 1GiB -c ws0k4-256j16 -v -t 5)
add_test(testspj2rangem spit -f wow -G 1GiB -c ms0k4-256j128 -v -t 5 )
add_test(testspj2rangewmg spit -f wow -G1 -c wms0k4-256j128 -v -t 5)
add_test(testsp2rangewmc spit -f wow -G 1GiB -c wms0k4:256j128 -v -t 3 )
add_test(testspitws1V  spit -f wow -G 1GiB -c ws1 -v -V -t 5)
add_test(testspitgenerate1 spit -G 1GiB -f wow -c k4-16P10000X1 -P p.txt -R1)
add_test(testspitgenerate1v spitchecker p.txt)
add_test(testspitgenerate2 spit -f wow -G 1GiB -c k4-16P1X1 -P p.txt -R1)
add_test(testspitgenerate2v spitchecker p.txt)
add_test(testspitmprefixj2norangewmu spit -f wow -G 0.5 -c ws0k4-8j2 -v -t 5)
add_test(testspitws1VV  spit -f wow -G 0.5 -c ws1 -v -VV -t 5)
add_test(testspitws1VVV  spit -f wow -G 0.5 -c ws1 -v -VVV -t 5)
add_test(testspitws1j0 spit -f wow -G 0.5 -c ws1j0 -v -t 1)
add_test(testspitws1j0u spit -f wow -G 0.5 -c ws1j0 -v -t 1 -u)
add_test(testspitws1j0U0 spit -f wow -G 0.5 -c ws1j0 -v -t 1 -U0)
add_test(testspitws1j1 spit -f wow -G 0.5 -c ws1j1 -v -t 1)
add_test(testspitws1rangeunderscore spit -f wow -G 0.5 -c ws1G0_0.25j5 -c ws1G0.25_0.5j8 -v  -t 5)
add_test(testspit_ws0j4 spit -f wow -G 1GiB -c ws0P1000j4  -v -t 5)
add_test(testspit_ws0j4e2b spit -e "2,echo wow" -f wow -G 1GiB -c ws0P1000j4  -v -t 5)
add_test(testspit_ws0j4e2a spit -f wow -G 1GiB -c ws0P1000j4  -v -t 5 -e "3,echo wow")
add_test(testspit_ws0j4to32 spit -f wow -G 1GiB -c ws0k4-32P1000j4  -v -t 5)
add_test(testspit_ws0j4to32c spit -f wow -G 1GiB -c ws0k4:2048P1000j4  -v -t 5)
add_test(testspitw  spit -f wow -G 1GiB -c w -v -t 5)
#add_test(testspitwDs0  spit -f wow -G 1GiB -c wDs0 -v -t 5)
add_test(testspit_precon spit -f wow -G 1GiB -pG -c ws0Nnj4 -v -t 5)
add_test(testspitw  spit -f wow -G 0.5-1 -c w -v -t 5)
add_test(testspitwB  spit -f wow -G 0.5-1 -c w -v -t 5 -B /tmp/$$)
add_test(testspitwBM  spit -f wow -G 0.5-1 -c w -v -t 5 -B /tmp/$$ -M options)
add_test(testspitwBMN  spit -f wow -G 0.5-1 -c w -v -t 5 -B /tmp/$$ -M options -N options)
add_test(testspitr  spit -f wow -G 1GiB -c r -v -t 5)
add_test(testspitm  spit -f wow -G 1GiB -c m -v -t 5)

add_test(testspitwx1  spit -f wow -G 1GiB -c ws0x1k4-16 -v -t 5)
add_test(testspitwX1  spit -f wow -G 1GiB -c ws0X1k4-16 -v -t 5)
add_test(testspitwx1n  spit -f wow -G 1GiB -c wx1n -v -t 5)
add_test(testspitwX1n  spit -f wow -G 1GiB -c wX1n -v -t 5)
add_test(testspitmprefix spit -f wow -G 1GiB -c m -v -t 5)
add_test(testspitmprefixj2 spit -f wow -G 1GiB -c mj2 -v -t 5)
add_test(testspitmprefixj2range spit -f wow -G 1GiB -c ws0k4-128j128 -v -t 30 )
add_test(testspitmprefixx1 spit -f wow -G 1GiB -c rwx1j2 -v)
add_test(testspitrw  spit -f wow -G 1GiB -c rw -v -t 5)
add_test(testspitp0_75 spit -f wow -G 1GiB -c p0.75 -v -t 5)
add_test(testspitp0_75m spit -f wow -G 1GiB -c p0.75m -v -t 5)
add_test(testspit_ws0 spit -f wow -G 1GiB -c ws0P1000j1 -v -t 5)
add_test(testspit_ws0j2 spit -f wow -G 1GiB -c ws0P1000j2 -v -t 5)
add_test(testspit_ws0 spit -f wow -G 1GiB -c ws0j2 -v -t 5)
add_test(testspit_ws1 spit -f wow -G 1GiB -c ws1 -v -t 5)
add_test(testspit_ws1j3 spit -f wow -G 1GiB -c ws1j3 -v -t 5)
add_test(testspit_ws32 spit -f wow -G 1GiB -c ws1 -v -t 5)
add_test(testspit_cr_cw spit -f wow -G 1GiB -c w -c r -v -t 5)
add_test(testspit_cr_cw spit -f wow -G 1GiB -c wj4 -v -t 5)
add_test(testspit_cr_cw spit -f wow -G 1GiB -c ms32j4 -v -t 5)
add_test(testspit_j2 spit -f wow -G 1GiB -c w -c j2r -v -t 5)
add_test(testspit_P1 spit -f wow -G 1GiB -c P1 -v -t 5)
add_test(testspit_P255 spit -f wow -G 1GiB -c P255 -v -t 5)
add_test(testspit_P100000 spit -f wow -G 1GiB -c P100000 -v -t 5)
add_test(testspit_P100000m spit -f wow -G 1GiB -c mP100000 -v -t 5)
add_test(testspit_B spit -f wow -G 1GiB -c P100000 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_B_two_piece  spit -f wow -G 1GiB -c rs0G0-0.5 -c rs1G0.5-1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_P  spit -f wow -G 1GiB -c rw -P -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_s0_1  spit -f wow -G 1GiB -c rw -s 0.1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4_1024  spit -f wow -G 1GiB -c rwk4-1024 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4_nN  spit -f wow -G 1GiB -c rs0nNP100 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4W  spit -f wow -G 1GiB -c wW0.1:0.1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4W2  spit -f wow -G 1GiB -c wW1:1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_J4  spit -f wow -G 1GiB -c ws0J4 -v -t 5 )
add_test(testspit_J4S100  spit -f wow -G 1GiB -c ws0J4S100 -v -t 5 )

#set (CTEST_TEST_TIMEOUT 1)
#add_test(testspit_fuzz spit fuzz wow)

set_property(
  TARGET spit
  APPEND
  PROPERTY ADDITIONAL_CLEAN_FILES spit-version.h
)
      
