FROM python:3.10-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    && rm -rf /var/lib/apt/lists/*

RUN pip install --upgrade pip

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY ssl_proxy.py .

EXPOSE 51234

CMD ["gunicorn", "--bind", "0.0.0.0:51234", "--workers", "2", "ssl_proxy:app"]
