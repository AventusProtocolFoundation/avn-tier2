#!/bin/bash

for file in $(find ${1-:.} -type f \( -iname "*.csv" ! -iname "*out_*" \))
do
  node $(dirname $(readlink -f $0 || realpath $0))/parseGrafanaData.js $file
done