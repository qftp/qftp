#!/usr/bin/env bash

if [ $# -ne 3 ]; then
    echo "Usage: $0 <size> <count> <target-dir>"
    exit 1
fi

size=$1
count=$2
dir=$3

mkdir -p "$dir"

for i in $(seq $count)
do
    head -c $size /dev/urandom > "${dir}/file${i}"
done
