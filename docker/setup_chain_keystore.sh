#!/bin/bash
# Setups the keystore for the validator nodes. Uses the scripts generated from avn-scripts. Arguments:
# $1 - the environment file that contains the environment variables.

for i in {0..4}
do
    # Some configurations create the keystore scripts to use other rpc port thatn 9933.
    # In containers & production we always use 9933, so we replace with that
    for j in avn-scripts-output/AvnValidator${i}/set_*; do
        sed -i 's/localhost:993./localhost:9933/g' $j
    done
    docker-compose --env-file ${1} exec -T validator-${i} /bin/sh /avn/keystore-scripts/set_imon_sessionKey.sh
    docker-compose --env-file ${1} exec -T validator-${i} /bin/sh /avn/keystore-scripts/set_babe_sessionKey.sh
    docker-compose --env-file ${1} exec -T validator-${i} /bin/sh /avn/keystore-scripts/set_gran_sessionKey.sh
    docker-compose --env-file ${1} exec -T validator-${i} /bin/sh /avn/keystore-scripts/set_audi_sessionKey.sh
    docker-compose --env-file ${1} exec -T validator-${i} /bin/sh /avn/keystore-scripts/set_ethk_sessionKey.sh
    docker-compose --env-file ${1} exec -T validator-${i} /bin/sh /avn/keystore-scripts/set_avnk_sessionKey.sh
done
