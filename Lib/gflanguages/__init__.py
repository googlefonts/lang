#!/usr/bin/env python3
#
# Copyright 2022 The Google Fonts Tools Authors.
# Copyright 2017,2022 Google LLC All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS-IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
"""
Helper API for interaction with languages/regions/scripts
data on the Google Fonts collection.
"""
import glob
import os
import unicodedata

from gflanguages import languages_public_pb2
from google.protobuf import text_format
from pkg_resources import resource_filename

try:
    from ._version import version as __version__  # type: ignore
except ImportError:
    __version__ = "0.0.0+unknown"

DATA_DIR = resource_filename("gflanguages", "data")


def _load_thing(thing_type, proto_class, base_dir=DATA_DIR):
    if base_dir is None:
        base_dir = DATA_DIR

    thing_dir = os.path.join(base_dir, thing_type)
    things = {}
    for textproto_file in glob.iglob(os.path.join(thing_dir, "*.textproto")):
        with open(textproto_file, "r", encoding="utf-8") as f:
            proto = proto_class()
            thing = text_format.Parse(f.read(), proto)
            assert thing.id not in things, f"Duplicate {thing_type} id: {thing.id}"
            things[thing.id] = thing
    return things


def LoadLanguages(base_dir=DATA_DIR):
    return _load_thing("languages", languages_public_pb2.LanguageProto, base_dir)


def LoadScripts(base_dir=DATA_DIR):
    return _load_thing("scripts", languages_public_pb2.ScriptProto, base_dir)


def LoadRegions(base_dir=DATA_DIR):
    return _load_thing("regions", languages_public_pb2.RegionProto, base_dir)


def parse(exemplars: str):
    """Parses a list of exemplar characters into a set of codepoints."""
    codepoints = set()
    for chars in exemplars.split():
        if len(chars) > 1:
            chars = chars.lstrip("{").rstrip("}")
        normalized_chars = unicodedata.normalize("NFC", chars)
        if normalized_chars != chars:
            for char in normalized_chars:
                codepoints.add(char)
        for char in chars:
            codepoints.add(char)
    return codepoints
