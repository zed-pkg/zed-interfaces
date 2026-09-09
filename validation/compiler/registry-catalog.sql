-- Metadata only; no user, session, package-content, or credential rows.
-- Run against the disposable database initialized from the pinned owned DDL.
\set ON_ERROR_STOP on
begin read only;
select jsonb_build_object(
  'schema', 'public',
  'tables', coalesce((
    select jsonb_object_agg(inventory.table_name, inventory.columns)
    from (
      select c.table_name,
        jsonb_object_agg(c.column_name, jsonb_build_object(
          'data_type', c.data_type,
          'nullable', c.is_nullable = 'YES'
        ) order by c.ordinal_position) as columns
      from information_schema.columns c
      join information_schema.tables t
        on t.table_schema = c.table_schema and t.table_name = c.table_name
      where c.table_schema = 'public'
        and left(c.table_name, 4) = 'zed_'
        and t.table_type = 'BASE TABLE'
      group by c.table_name
    ) inventory
  ), '{}'::jsonb)
);
commit;
