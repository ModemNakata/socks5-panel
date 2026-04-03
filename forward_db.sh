#!/bin/bash
# port changed
# ssh -L 5431:127.0.0.1:5431 nakata # foreground
ssh -f -N -L 5431:127.0.0.1:5431 nakata # background

# -f background -N no shell
