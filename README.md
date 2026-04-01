# Todo API

## Environment variables

| Variable               | Description                                       |
| ---------------------- | ------------------------------------------------- |
| `PORT`                 | Port the app should listen on, defaults to `3000` |
| `APP_NAME`             | Name of the app, defaults to `my-app`             |
| `POSTGRESQL_ADDON_URI` | PostgreSQL connection uri                         |

## CleverCloud deployment

```sh
$ clever create --type rust todo-drichard
$ clever addon create postgresql-addon --plan dev pg-todo-drichard
$ clever service link-addon pg-todo-drichard
$ clever env set APP_NAME drichard
$ clever deploy
```
