# Recovered tuning configs

Extracted verbatim from the shipped Burgerlib rez archive
(`astrorock-tools extract-rez`) — these are the original 1997 per-level
tuning tables, which have no loose source file in the reference tree.

Format is documented by each system's `XxxInit()` parser in the original
C++ (e.g. `rocks.cpp`: `level:count,` per line). `goodies.cfg` starts
with a human-readable column-header line the original parser skips.

| file | rez resource | parsed by |
|---|---|---|
| rocks.cfg | rRocksCfg (15) | rocks.cpp |
| gloops.cfg | rGloopCfg (19) | gloops.cpp |
| spikeball.cfg | rSpikeBallCfg (23) | SpikeBall.cpp |
| hks.cfg | rHksCfg (28) | hk.cpp |
| bomber.cfg | rBomberCfg (34) | bomber.cpp |
| fastdeth.cfg | rFastDethCfg (41) | fastdeth.cpp |
| goodies.cfg | rGoodiesCfg (43) | goodies.cpp |

Do not hand-edit without noting the change here — gameplay balance is
part of the port's fidelity contract.
