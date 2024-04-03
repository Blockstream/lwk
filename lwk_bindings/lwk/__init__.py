from .lwk import *  # NOQA

__doc__ = lwk.__doc__
if hasattr(lwk, "__all__"):
    __all__ = lwk.__all__

# TODO: `help(lwk)` shows nested packages like `lwk.lwk.lwk.Address` even though
# they are available with just `lwk.Address`

