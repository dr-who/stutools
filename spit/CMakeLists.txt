cmake_minimum_required (VERSION 2.8)
project (stutools)

get_directory_property(hasParent PARENT_DIRECTORY)
if(NOT hasParent)
  include( ${CMAKE_CURRENT_SOURCE_DIR}/../utils/Version.cmake)
endif()
  

include (CTest)

#enable_testing()

set( CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -Werror -Wextra -Wall -pedantic --std=gnu11 -O2 " )
#SET (CMAKE_C_COMPILER             "/usr/bin/clang")

add_library(spitlib STATIC positions.c devices.c utils.c diskStats.c logSpeed.c aioRequests.c jobType.c histogram.c spitfuzz.c blockVerify.c lengths.c workQueue.c list.c latency.c)

add_executable(spit spit.c)
target_link_libraries(spit spitlib m aio pthread numa)

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

add_executable(fsfiller fsfiller.c)
target_link_libraries(fsfiller spitlib m aio pthread numa)

add_executable(spitchecker verify.c)
target_link_libraries(spitchecker spitlib m aio pthread numa)

add_executable(raidcorrupt raidcorrupt.c)
target_link_libraries(raidcorrupt spitlib m aio pthread numa)

add_executable(devcontents devcontents.c)
target_link_libraries(devcontents spitlib m aio pthread numa)

add_executable(bdinfo bdinfo.c)
target_link_libraries(bdinfo spitlib m aio pthread numa)

add_executable(dtest directories.c)
target_link_libraries(dtest spitlib m aio pthread numa)

add_executable(hist hist.c)
target_link_libraries(hist spitlib m numa)

add_executable(genpositions genpositions.c)
target_link_libraries(genpositions spitlib m aio pthread numa)


#add_executable(testpositions testpositions.c)
#target_link_libraries(testpositions spitlib m pthread aio)

add_executable(testDiskStats testDiskStats.c)
target_link_libraries(testDiskStats spitlib m numa pthread)

install(TARGETS spit fsfiller spitchecker raidcorrupt bdinfo dtest hist DESTINATION bin)

# first check the position validation/collision collapsing is working
#add_test(tds testDiskStats)


add_test(testspitHelp  spit)
add_test(testspitfail1  spit -c r)
set_tests_properties(testspitfail1 PROPERTIES WILL_FAIL TRUE )

add_test(testspitfail2  spit -f /dev/wowwww)
set_tests_properties(testspitfail2 PROPERTIES WILL_FAIL TRUE )

add_test(testspitfail3  spit -r)
set_tests_properties(testspitfail3 PROPERTIES WILL_FAIL TRUE )

add_test(testspitwk512b  spit -f wow -G 0.5 -c ws1k0.5 -v -V -t 1)

add_test(testspita10 spit -f wow -G 1 -c ws1a10zM1 -v -t 5 )

add_test(testspitrange1 spit -f wow -G 1 -c rM1024q1j1x1 -v  )
add_test(testspitrange2 spit -f wow -G 1 -c rM1024q1j2x1oG_ -v  )
set_tests_properties(testspitrange2 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange3 spit -f wow -G 1 -c rM1025q1x1 -v  )
set_tests_properties(testspitrange3 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange4 spit -f wow -G 0.5-1 -c rM513q1x1 -v  )
set_tests_properties(testspitrange4 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange5 spit -f wow -G 0.5-1 -c rM512q1x1 -v  )
add_test(testspitrange6 spit -f wow -G 0.5-1 -c rM256q1x1j2G_ -v  )

add_test(testspitrange7 spit -f wow -G 0.5-1 -c rM257q1x1j2G_ -v  )
set_tests_properties(testspitrange7 PROPERTIES WILL_FAIL TRUE)

add_test(testspitrange8 spit -f wow -G 1 -c rM4096 -v  )
set_tests_properties(testspitrange8 PROPERTIES WILL_FAIL TRUE)

add_test(testspita10x3n spit -f wow -G 1 -c ws1zM1x3n -v -t 5 )

add_test(testspitPlotsa spit -f wow -G 1 -c ws0P10000M64j16G_ -v -t 5 )
add_test(testspitPlotsb spit -f wow -G 1 -c ws0M64j16G_P100000 -v -t 5 )
add_test(testspitPlotsc spit -f wow -G 1 -c ws0k8j16G_P10000 -v -t 5 )
add_test(testspitPlotsb spit -f wow -G 1 -c ws0j16G_P10000k8 -v -t 5 )

add_test(testspitpf10 spit -p f10 -f wow -G 1 -c w -v -t 5)
add_test(testspitwm1 spit -f wow -G 1 -c wm1s1 -v -t 5)
add_test(testspitwm2 spit -f wow -G 1 -c wm2s1 -v -t 5)
add_test(testspitwm4 spit -f wow -G 1 -c wm4s1 -v -t 5)
add_test(testspitwm4q2 spit -f wow -G 1 -c wm4s1q2 -v -t 5)
add_test(testspitwm4K0 spit -f wow -G 1 -c ws1zk4K0x1 -v -t 5)
add_test(testspitwm4K1 spit -f wow -G 1 -c ws1zk4K1x1 -v -t 5)
add_test(testspitwm4K10 spit -f wow -G 1 -c ws1zk4K10x1 -v -t 5)
add_test(testspitwm4q6 spit -f wow -G 1 -c wm4s1q6 -v -t 5)
add_test(testspitwm_1 spit -f wow -G 1 -c wm-1s1 -v -t 5)

add_test(testspitF1noj spit -F wowf -G 1 -c ws1 -v -t 5)
add_test(testspitF2j1 spit -F wowf -G 1 -c ws1j1 -v -t 5)
add_test(testspitF3j2 spit -F wowf -G 0.5 -c ws1j2 -v -t 5)
add_test(testspitF4j4 spit -F wowf -G 0.25 -c ws1j2 -v -t 5)

add_test(testspitFS100 spit -f wow -G 1 -c ws1j2S100 -v -t 5)
add_test(testspitFS100_10 spit -f wow -G 1 -c ws1j2S100-10 -v -t 5)

add_test(testspitmprefixj2rangewmUj1 spit -f wow -G 1 -c wUs0k4-256 -v -t 5 )
add_test(testspitmprefixj2rangewmP1000 spit -f wow -G 1 -c ws0k8-16P1000 -v -t 5 )
add_test(testspitmprefixj2rangewmP1000j17 spit -f wow -G 1 -c ws0k8-16P1000G_j17u -v -t 5 )
add_test(testspitmprefixj2rangewm spit -f wow -G 1 -c ws0k4-256j16G_ -v -t 5)
add_test(testspitmprefixj2rangem spit -f wow -G 1 -c G_ms0k4-256j128 -v -t 5 )
add_test(testspitmprefixj2rangewm spit -f wow -G1 -c G_wms0k4-256j128 -v -t 5)
add_test(testspitmprefixj2rangewmc spit -f wow -G 1 -c G_wms0k4:256j128 -v -t 3 )
add_test(testspitws1V  spit -f wow -G 1 -c ws1 -v -V -t 5)
add_test(testspitgenerate1 spit -G 1 -f wow -c k4-16P10000 -P p.txt)
add_test(testspitgenerate1v spitchecker p.txt)
add_test(testspitgenerate2 spit -f wow -G 1 -c k4-16P1 -P p.txt)
add_test(testspitgenerate2v spitchecker p.txt)
add_test(testspitmprefixj2norangewmu spit -f wow -G 0.5 -c ws0k4-8j2 -v -t 5)
add_test(testspitws1VV  spit -f wow -G 0.5 -c ws1 -v -VV -t 5)
add_test(testspitws1VVV  spit -f wow -G 0.5 -c ws1 -v -VVV -t 5)
add_test(testspitws1j0 spit -f wow -G 0.5 -c ws1j0 -v -t 1)
add_test(testspitws1j0u spit -f wow -G 0.5 -c ws1j0 -v -t 1 -u)
add_test(testspitws1j0U0 spit -f wow -G 0.5 -c ws1j0 -v -t 1 -U0)
add_test(testspitws1j1 spit -f wow -G 0.5 -c ws1j1 -v -t 1)
add_test(testspitws1rangeunderscore spit -f wow -G 0.5 -c ws1G0_0.25j5 -c ws1G0.25_0.5j8 -v  -t 5)
add_test(testspit_ws0j4 spit -f wow -G 1 -c ws0P1000 -j4  -v -t 5)
add_test(testspit_ws0j4e2b spit -e "2,echo wow" -f wow -G 1 -c ws0P1000 -j4  -v -t 5)
add_test(testspit_ws0j4e2a spit -f wow -G 1 -c ws0P1000 -j4  -v -t 5 -e "3,echo wow")
add_test(testspit_ws0j4to32 spit -f wow -G 1 -c ws0k4-32P1000 -j4  -v -t 5)
add_test(testspit_ws0j4to32c spit -f wow -G 1 -c ws0k4:2048P1000 -j4  -v -t 5)
add_test(testspitw  spit -f wow -G 1 -c w -v -t 5)
add_test(testspitwDs0  spit -f wow -G 1 -c wDs0 -v -t 5)
add_test(testspit_precon spit -f wow -G 1 -pG -c ws0NnG_ -j4 -v -t 5)
add_test(testspitw  spit -f wow -G 0.5-1 -c w -v -t 5)
add_test(testspitwB  spit -f wow -G 0.5-1 -c w -v -t 5 -B /tmp/$$)
add_test(testspitwBM  spit -f wow -G 0.5-1 -c w -v -t 5 -B /tmp/$$ -M options)
add_test(testspitwBMN  spit -f wow -G 0.5-1 -c w -v -t 5 -B /tmp/$$ -M options -N options)
add_test(testspitr  spit -f wow -G 1 -c r -v -t 5)
add_test(testspitm  spit -f wow -G 1 -c m -v -t 5)

add_test(testspitwx1  spit -f wow -G 1 -c ws0x1k4-16 -v -t 5)
add_test(testspitwX1  spit -f wow -G 1 -c ws0X1k4-16 -v -t 5)
add_test(testspitwx1n  spit -f wow -G 1 -c wx1n -v -t 5)
add_test(testspitwX1n  spit -f wow -G 1 -c wX1n -v -t 5)
add_test(testspitmprefix spit -f wow -G 1 -c m -v -t 5)
add_test(testspitmprefixj2 spit -f wow -G 1 -c m -v -t 5 -j2)
add_test(testspitmprefixj2range spit -f wow -G 1 -c ws0k4-128 -v -t 30 -j128)
add_test(testspitmprefixx1 spit -f wow -G 1 -c rwx1 -v -j2)
add_test(testspitrw  spit -f wow -G 1 -c rw -v -t 5)
add_test(testspitp0_75 spit -f wow -G 1 -c p0.75 -v -t 5)
add_test(testspitp0_75m spit -f wow -G 1 -c p0.75m -v -t 5)
add_test(testspit_ws0 spit -f wow -G 1 -c ws0P1000 -j1 -v -t 5)
add_test(testspit_ws0j2 spit -f wow -G 1 -c ws0P1000 -j2 -v -t 5)
add_test(testspit_ws0 spit -f wow -G 1 -c ws0G_j2 -v -t 5)
add_test(testspit_ws1 spit -f wow -G 1 -c ws1 -v -t 5)
add_test(testspit_ws1j3 spit -f wow -G 1 -c ws1G_j3 -v -t 5)
add_test(testspit_ws32 spit -f wow -G 1 -c ws1 -v -t 5)
add_test(testspit_cr_cw spit -f wow -G 1 -c w -c r -v -t 5)
add_test(testspit_cr_cwG_ spit -f wow -G 1 -c wG_j4 -v -t 5)
add_test(testspit_cr_cw spit -f wow -G 1 -c ms32G_j4 -v -t 5)
add_test(testspit_j2 spit -f wow -G 1 -c wG_ -c r -j2 -v -t 5)
add_test(testspit_P1 spit -f wow -G 1 -c P1 -v -t 5)
add_test(testspit_P255 spit -f wow -G 1 -c P255 -v -t 5)
add_test(testspit_P100000 spit -f wow -G 1 -c P100000 -v -t 5)
add_test(testspit_P100000m spit -f wow -G 1 -c mP100000 -v -t 5)
add_test(testspit_B spit -f wow -G 1 -c P100000 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_B_two_piece  spit -f wow -G 1 -c rs0G0-0.5 -c rs1G0.5-1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_P  spit -f wow -G 1 -c rw -P -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_s0_1  spit -f wow -G 1 -c rw -s 0.1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4_1024  spit -f wow -G 1 -c rwk4-1024 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4_nN  spit -f wow -G 1 -c rs0nNP100 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4W  spit -f wow -G 1 -c wW0.1:0.1 -v -t 5 -B /tmp/spit-tmp.tmp)
add_test(testspit_k4W2  spit -f wow -G 1 -c wW1:1 -v -t 5 -B /tmp/spit-tmp.tmp)

#set (CTEST_TEST_TIMEOUT 1)
#add_test(testspit_fuzz spit fuzz wow)
