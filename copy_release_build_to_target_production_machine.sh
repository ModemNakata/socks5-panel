#!/bin/bash
scp panel/target/release/c2-dashboard spb11:/socks5.website/panel/target/release/
# sync other necessary files
# scp .env spb11:/socks5.website/ # copy variables manaully
#scp docker-compose.yml spb11:/socks5.website/
scp -r panel/assets/. spb11:/socks5.website/assets/.

# don't copy over again, because db_url is changed from docker network to localhost / .env moved / env_path changed
# scp -r bg_charger spb11:/socks5.website/.
