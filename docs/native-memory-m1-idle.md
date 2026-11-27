# M1 idle RSS follow-up

The first matched Python wheels run (46030635199, artifacts 15214113265,
15214880885, 15214865817) passes all original growth/HWM/wall checks and
72 independent witnessed solves, but reveals a separate absolute idle RSS
regression. QPE improves median postsolve RSS from 113672 to 53432 KiB;
Grover increases 55180 to 86840 KiB and QNN increases 34968 to 106406 KiB.
The median free arena rises from 29.17 to 60.13 MiB for Grover and from
9.46 to 79.48 MiB for QNN, whereas the live-allocation proxies remain almost
unchanged (about 4.25 and 4.21 MiB). Thus a low growth measurement alone
does not establish low retained RSS: the warmup can already retain a large
arena. These are allocator diagnostics, not a proof of a leak or a
specific allocation stack causing the retention.

The next PR run compares the same default-off feature with the default
core on Grover and QNN only. Build symbolized local wheel pair once, verify
source and wheel SHA256, use separate virtual environments. Each family
runs two fresh processes per arm, 12 solves per process, reversing order
in block 1. Preserve the original 32 MiB growth, non-regressing HWM, 5%
median wall gate and exact binary / independent witness. Additionally
report absolute maximum and steady-state median postsolve RSS
nonregression for each pair (not merely warmup-relative growth).
Record USS, free arena, live proxy. Separate one-solve Heaptrack traces
after the uninstrumented runs capture allocation call counts and peak live
heap. Profiling timing is not native performance evidence.

This is a diagnostic gate: REVIEW is evidence, never admission. No
production solver changes, allocator tuning, trimming, larger pool, or
worker recycling. Retain QPE's qualified evidence on the previous commit,
recheck QPE and all other families if the solver is modified. M4 HOLD.
