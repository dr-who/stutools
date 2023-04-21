
 ./blockdevdiff /dev/older /dev/newer  > delta.now
 ./blockdevdiff /dev/newer < delta.now      # complains as the blocks are already there
 ./blockdevdiff /dev/older < delta.now      # updates the blocks

