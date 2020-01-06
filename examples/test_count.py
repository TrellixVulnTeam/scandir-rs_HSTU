import time
import sys

import scandir_rs as r

"""
GC support currently NOT working!

import gc

def test():
    C = r.count.Count("~/workspace", metadata_ext=True)
    C.start()
    del C

test()

gc.collect()
sys.exit()
"""

C = r.count.Count("~/workspace", metadata_ext=True)

with C:
    while C.busy():
        print(C.statistics)
        time.sleep(0.1)
print("FINISHED")
print(C.statistics)
print(r.count.Count("~/workspace", metadata_ext=True).collect())
print(r.count.count("~/workspace", metadata_ext=True))

C = r.count.Count("~/workspace", metadata_ext=True)
C.start()
print(C.busy())
print(C.statistics)
time.sleep(0.5)
print(C.busy())
print(C.statistics)
C.stop()
print(C.busy())
print(C.statistics)
print(C.duration)
print(C.has_results())
print(C.as_dict())
