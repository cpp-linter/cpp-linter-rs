# type: ignore
# ruff: noqa: F405 F403
import sys
from .cpp_linter import *

__doc__ = cpp_linter.__doc__
if hasattr(cpp_linter, "__all__"):
    __all__ = list(filter(lambda x: x != "main", cpp_linter.__all__))


def main():
    """The main entrypoint for the python frontend. See our rust docs for more info on
    the backend (implemented in rust)"""
    sys.exit(cpp_linter.main(sys.argv))
