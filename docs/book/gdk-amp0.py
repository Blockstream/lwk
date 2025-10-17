import green_gdk as gdk  # pip install green_gdk

gdk.init({"log_level": "debug", "datadir": "/tmp/test/gdk"})

network = "testnet-liquid" # "liquid" or "localtest-liquid"

username = ""
password = ""
mnemonic = ""  # gdk.generate_mnemonic_12()

sk_credentials = {"mnemonic": mnemonic}
wo_credentials = {"username": username, "password": password}

session = gdk.Session({"name": network})  # connect to green backend
session.register_user({}, sk_credentials).resolve()  # register wallet
session.login_user({}, sk_credentials).resolve()  # login and authenticate using the mnemonic
session.create_subaccount({"name": "amp", "type": "2of2_no_recovery"}).resolve()  # create AMP account
session.register_user({}, wo_credentials).resolve()  # register watch only

session_wo = gdk.Session({"name": network})  # connect to green backend
session_wo.login_user({}, wo_credentials).resolve()  # login with watch only credentials
