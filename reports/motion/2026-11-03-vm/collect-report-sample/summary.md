# Liubai on real hardware

- Machine: QEMU Standard PC (i440FX + PIIX, 1996); Intel(R) Core(TM) i5-3210M CPU @ 2.50GHz; 2963 MiB
- OS: Arch Linux, kernel 7.2.14-arch1-1
- GPU card1: 0x1af4:0x1050, driver virtio-pci
- Screen Virtual-1: 1440×900 at 75.0 Hz, scale 1.0
- niri 26.04 (8ed0da4); Liubai ?; renderer cairo (the shell's default)

## Memory (PSS, MiB)

| Moment | Whole desktop | Shell | Shell RSS | niri |
| --- | --- | --- | --- | --- |
| Idle | 166.8 | 22.9 | 40.7 | 77.9 |
| Today open | 179.4 | 30.8 | 53.7 | 82.7 |
| 30 s after closing | 169.1 | 25.2 | 43.4 | 77.9 |

Budget: whole desktop ≤ 400, shell ≤ 100; first-open caches ≤ 5 over idle.

## Frames while panels open and close (ms between frames)

| Panel | Drawing | Frames | Screen refresh | Median | 90 % | Longest | Late frames |
| --- | --- | --- | --- | --- | --- | --- | --- |
| today | cairo | 18 | 13.3 | 15.4 | 27.1 | 75.2 | 3 |
| search | cairo | 66 | 13.3 | 13.3 | 18.9 | 50.0 | 7 |
| notification | cairo | 29 | 13.3 | 13.3 | 14.1 | 19.9 | 0 |
| osd | cairo | 30 | 13.3 | 13.3 | 14.2 | 15.8 | 0 |
| today | gl | 22 | 13.3 | 13.4 | 16.0 | 83.3 | 3 |
| search | gl | 60 | 13.3 | 13.3 | 14.0 | 168.8 | 3 |
| notification | gl | 17 | 13.3 | 13.1 | 25.8 | 152.3 | 3 |
| osd | gl | 21 | 13.3 | 13.4 | 15.4 | 119.0 | 1 |

Smooth means the median equals the screen refresh and few frames are late.
cairo is the default (less memory); if gl is clearly smoother here, Liubai
should use it on machines like this one (decision D50).
