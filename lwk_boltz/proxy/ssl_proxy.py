from flask import Flask, request, json, Response
from flask_cors import CORS
import requests
import logging

app = Flask(__name__)
CORS(app)  # Allow all origins

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


@app.route('/proxy', methods=['GET', 'POST', 'PUT', 'DELETE'])
def proxy_request():
    target_url = request.headers.get('X-Proxy-URL')

    if not target_url:
        return Response(
            json.dumps({"error": "No target URL provided"}),
            status=400,
            mimetype='application/json'
        )

    try:
        # Log the incoming request details
        logger.info(f"Proxy request to: {target_url}")
        logger.info(f"Request method: {request.method}")
        logger.info(f"Request headers: {dict(request.headers)}")

        # Try to log request body
        try:
            request_body = request.get_json(silent=True)
            logger.info(f"Request body: {request_body}")
        except Exception as body_log_error:
            logger.error(f"Could not log request body: {body_log_error}")

        # Prepare headers, excluding Flask-specific ones
        headers = {
            key: value for (key, value) in request.headers
            if key not in ['Host', 'X-Proxy-URL', 'Content-Length']
        }

        # Determine the method and make the request
        methods = {
            'GET': requests.get,
            'POST': requests.post,
            'PUT': requests.put,
            'DELETE': requests.delete
        }

        method = methods.get(request.method)
        if not method:
            return Response(
                json.dumps({"error": "Unsupported HTTP method"}),
                status=405,
                mimetype='application/json'
            )

        # Prepare request arguments
        kwargs = {
            'url': target_url,
            'headers': headers,
            'verify': False  # Bypass SSL verification
        }

        # Add data for methods that support it
        if request.method in ['POST', 'PUT']:
            # Try to parse request data as JSON if possible
            request_json = request.get_json(silent=True)
            if request_json:
                kwargs['json'] = request_json
            else:
                kwargs['data'] = request.get_data()

        # Make the request
        try:
            response = method(**kwargs)
        except Exception as request_error:
            logger.error(f"Request error: {request_error}")
            return Response(
                json.dumps({
                    "error": "Failed to make request to target service",
                    "details": str(request_error)
                }),
                status=500,
                mimetype='application/json'
            )

        return Response(
            response.text,
            status=response.status_code,
            mimetype='application/json'
        )

    except Exception as e:
        logger.error(f"Unexpected error: {e}")
        return Response(
            json.dumps({"error": str(e)}),
            status=500,
            mimetype='application/json'
        )


if __name__ == '__main__':
    app.run(host='0.0.0.0', port=51234)
