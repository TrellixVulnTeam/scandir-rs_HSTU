# -*- coding: utf-8 -*-

import os
import time
import sys

import scandir_rs as scandir

dirName = "C:/Windows" if os.name == 'nt' else "/usr"
print(scandir.count.Count(dirName).collect())
# Output is something like:
# Statistics { dirs: 76923, files: 648585, slinks: 48089,
#              hlinks: 0, devices: 0, pipes: 0, size: 0, usage: 0,
#              errors: [], duration: 1.07 }

print(scandir.count.Count(dirName, extended=True).collect())
# Output is something like:
# Statistics { dirs: 76923, files: 648585, slinks: 48089,
#              hlinks: 1113, devices: 0, pipes: 0, size: 32448804258,
#              usage: 34060193792, errors: [], duration: 0.934 }
