#!/bin/bash

touch inline-elision-eval.frg
cat $1 >> inline-elision-eval.frg # analysis_result
cat basic-helpers.frg >> inline-elision-eval.frg
cat $2 >> inline-elision-eval.frg # property
racket inline-elision-eval.frg
