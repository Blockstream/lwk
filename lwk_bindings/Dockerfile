# the Pyo3 provided docker lacks the uniffi bindgen installed from pip or available in the pat
# note it must be the exact same version used in Cargo.toml

FROM ghcr.io/pyo3/maturin

RUN python3.8 -m pip install --no-cache-dir uniffi-bindgen==0.26.1 \
    && python3.9 -m pip install --no-cache-dir uniffi-bindgen==0.26.1 \
    && python3.10 -m pip install --no-cache-dir uniffi-bindgen==0.26.1 \
    && python3.11 -m pip install --no-cache-dir uniffi-bindgen==0.26.1 \
    && python3.12 -m pip install --no-cache-dir uniffi-bindgen==0.26.1 

