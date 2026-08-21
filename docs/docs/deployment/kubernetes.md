---
sidebar_position: 3
---

# Kubernetes Deployment

Codex is designed for horizontal scaling in Kubernetes.

## Architecture

```
                    ┌───────────────────────┐
                    │    Load Balancer      │
                    └───────────┬───────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
        ▼                       ▼                       ▼
┌───────────────┐       ┌───────────────┐       ┌───────────────┐
│  Codex Pod 1  │       │  Codex Pod 2  │       │  Codex Pod N  │
│  + Workers    │       │  + Workers    │       │  + Workers    │
└───────┬───────┘       └───────┬───────┘       └───────┬───────┘
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │      PostgreSQL       │
                    │   (Single Instance)   │
                    └───────────────────────┘
```

## Prerequisites

- Kubernetes cluster (1.21+)
- PostgreSQL database (required for multi-replica)
- Shared storage (NFS, CephFS, or cloud storage)
- kubectl configured

## Deployment Manifest

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: codex
  labels:
    app: codex
spec:
  replicas: 3
  selector:
    matchLabels:
      app: codex
  template:
    metadata:
      labels:
        app: codex
    spec:
      initContainers:
        # Fail the pod before the app container starts if any CODEX_ variable
        # is misspelled or is not read by this version. Needs no database, so
        # it runs whether or not PostgreSQL is up yet.
        #
        # It must be given the SAME environment as the codex container below,
        # otherwise it validates something the app will never see.
        - name: config-check
          image: codex:latest
          args: ["config", "check", "--strict", "--quiet"]
          env:
            - name: CODEX_DATABASE__DB_TYPE
              value: "postgres"
            - name: CODEX_DATABASE__POSTGRES__HOST
              valueFrom:
                configMapKeyRef:
                  name: codex-config
                  key: postgres-host
            - name: CODEX_AUTH__JWT_SECRET
              valueFrom:
                secretKeyRef:
                  name: codex-secrets
                  key: jwt-secret
      containers:
        - name: codex
          image: codex:latest
          ports:
            - containerPort: 8080
          env:
            - name: CODEX_DATABASE__DB_TYPE
              value: "postgres"
            - name: CODEX_DATABASE__POSTGRES__HOST
              valueFrom:
                configMapKeyRef:
                  name: codex-config
                  key: postgres-host
            - name: CODEX_DATABASE__POSTGRES__PASSWORD
              valueFrom:
                secretKeyRef:
                  name: codex-secrets
                  key: postgres-password
            - name: CODEX_AUTH__JWT_SECRET
              valueFrom:
                secretKeyRef:
                  name: codex-secrets
                  key: jwt-secret
          volumeMounts:
            - name: library
              mountPath: /library
              readOnly: true
            - name: thumbnails
              mountPath: /app/data/thumbnails
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 5
          resources:
            requests:
              memory: "256Mi"
              cpu: "250m"
            limits:
              memory: "1Gi"
              cpu: "1000m"
      volumes:
        - name: library
          persistentVolumeClaim:
            claimName: library-pvc
        - name: thumbnails
          persistentVolumeClaim:
            claimName: thumbnails-pvc
```

## Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: codex
spec:
  selector:
    app: codex
  ports:
    - port: 80
      targetPort: 8080
  type: ClusterIP
```

## Ingress

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: codex
  annotations:
    # For SSE support - disable buffering
    nginx.ingress.kubernetes.io/proxy-buffering: "off"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
spec:
  rules:
    - host: codex.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: codex
                port:
                  number: 80
  tls:
    - hosts:
        - codex.example.com
      secretName: codex-tls
```

## ConfigMap and Secrets

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: codex-config
data:
  postgres-host: "postgres.default.svc.cluster.local"
---
apiVersion: v1
kind: Secret
metadata:
  name: codex-secrets
type: Opaque
stringData:
  postgres-password: "your-secure-password"
  jwt-secret: "your-jwt-secret"
```

## Storage Considerations

### Shared Storage

All pods need access to:
- **Media library**: ReadOnlyMany (ROX) or ReadWriteMany (RWX) PVC
- **Thumbnails**: ReadWriteMany (RWX) PVC for shared cache

Storage options:
- NFS
- CephFS
- Cloud storage (EFS, Azure Files, GCP Filestore)

### PersistentVolumeClaim Example

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: library-pvc
spec:
  accessModes:
    - ReadOnlyMany
  storageClassName: nfs
  resources:
    requests:
      storage: 1Ti
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: thumbnails-pvc
spec:
  accessModes:
    - ReadWriteMany
  storageClassName: nfs
  resources:
    requests:
      storage: 50Gi
```

## Database Requirements

- PostgreSQL is **required** for multi-replica deployments
- Use a managed PostgreSQL service or StatefulSet
- Ensure connection pooling for many replicas

## Session Handling

- JWT tokens are stateless, so authenticating a request needs no server-side lookup
- Multi-step sign-in flows keep their in-flight state in the database, not in the
  pod that started them
- Load balancing therefore works without session affinity, and no sticky sessions
  are needed

The second point is the one that is easy to lose. A few flows span two requests
that the load balancer is free to send to different pods: OIDC sign-on issues an
authorization request and later receives a callback, and connecting a user plugin
over OAuth does the same. The server-side half of those flows (the CSRF state, the
PKCE verifier, the nonce) lives in the database precisely so the pod handling the
callback does not have to be the pod that started the flow.

Codex 2.0.0 and earlier held that state in memory. On those versions, running more
than one replica without session affinity fails every OIDC login and every plugin
OAuth connect with `Invalid or expired OIDC state`, because the two legs are
consecutive requests and round-robin sends them to different pods almost every
time. Either upgrade or run a single replica.

Entity change events are bridged between replicas over PostgreSQL `LISTEN`/`NOTIFY`
so that a change made through one pod reaches the SSE subscribers and search index
of the others. This is another reason multi-replica deployments require PostgreSQL.

If you add server-side state that has to survive from one request to the next, put
it in the database. Anything held in a pod's memory silently reintroduces a need
for session affinity that nothing here enforces.

## Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: codex-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: codex
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

## Helm Chart (Coming Soon)

A Helm chart for easier deployment is planned.
