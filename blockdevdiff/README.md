
 ./blockdevdiff /dev/older /dev/newer  > delta.now
 
 ./blockdevreplay /dev/newer < delta.now      # complains as the blocks are already there
 
 ./blockdevreplay /dev/older < delta.now      # updates the blocks

 cat delta.* | ./blockdevreplay /dev/older    # updates /dev/older in place (overwrites)

