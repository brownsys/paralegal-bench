#!/bin/bash

# SPDX-FileCopyrightText: 2023 Aravinth Manivannan <realaravinth@batsense.net>
#
# SPDX-License-Identifier: AGPL-3.0-or-later


# added by <david_fryd@brown.edu>
# Determine OS and set the command
# for sha256sum accordingly
if [[ "$OSTYPE" == "darwin"* ]]; then
  # For macOS:
  SHASUM_CMD="shasum -a 256"
else
  # For Linux and other systems with sha256sum available:
  SHASUM_CMD="sha256sum"
fi

set -Eeuo pipefail
trap cleanup SIGINT SIGTERM ERR EXIT

readonly PROJECT_ROOT=$(realpath $(dirname $(dirname "${BASH_SOURCE[0]}")))
source $PROJECT_ROOT/scripts/lib.sh

readonly DIST=$PROJECT_ROOT/static/cache/bundle/


file_extension() {
	echo $1 | rev | tr
}

cache_bust(){
	name=$(get_file_name $1)
	extension="${name##*.}"
	filename="${name%.*}"
	file_hash=$($SHASUM_CMD $1 | cut -d " " -f 1 | tr "[:lower:]" "[:upper:]")

	msg "${GREEN}- Processing $name: $filename.$file_hash.$extension"

	sed -i ''\
		"s/$name/assets\/bundle\/$filename.$file_hash.$extension/" \
		$(find $DIST -type f -a -name "*.js")
	# macOS requires explicit empty string arg to -i, in Linux its ignored
}

setup_colors

msg "${BLUE}[*] Setting up files for cache busting"

for file in $(find $DIST  -type f -a -name "*.js")
do
	name=$(get_file_name $file)
	case $name in
		"bench.js")
			cache_bust $file
			;;
	esac
done
