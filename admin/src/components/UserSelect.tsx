import { Select } from 'antd';
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api } from '../api/client';
import type { Paginated } from '../api/types/Paginated';
import type { UserDto } from '../api/types/UserDto';

export function UserSelect({
  role,
  value,
  onChange,
  allowClear = true,
  placeholder = 'Пользователь',
}: {
  role?: string;
  value?: string;
  onChange: (v?: string) => void;
  allowClear?: boolean;
  placeholder?: string;
}) {
  const [search, setSearch] = useState('');
  const { data, isFetching } = useQuery({
    queryKey: ['user-select', role, search],
    queryFn: () =>
      api.get<Paginated<UserDto>>(
        `/api/admin/users?per_page=20${role ? `&role=${role}` : ''}${search ? `&q=${encodeURIComponent(search)}` : ''}`,
      ),
  });
  return (
    <Select
      showSearch
      allowClear={allowClear}
      filterOption={false}
      placeholder={placeholder}
      value={value}
      loading={isFetching}
      onSearch={setSearch}
      onChange={(v) => onChange(v ?? undefined)}
      options={data?.items.map((u) => ({ value: u.id, label: `${u.full_name} (${u.login})` }))}
      style={{ minWidth: 220 }}
    />
  );
}
