FROM python:3.12-slim
WORKDIR /app
COPY ai_service/requirements.txt ./requirements.txt
RUN pip install --no-cache-dir -r requirements.txt
COPY ai_service ./ai_service
USER 65532:65532
EXPOSE 8090
CMD ["python", "-m", "uvicorn", "ai_service.main:app", "--host", "0.0.0.0", "--port", "8090"]
