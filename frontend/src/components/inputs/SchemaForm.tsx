// frontend/src/components/inputs/SchemaForm.tsx
// JSON Schema → form renderer. Mirrors the Dioxus schema_form.rs behavior:
//   string          → text Input (Select dropdown when `enum` present)
//   number/integer  → number Input
//   boolean         → Checkbox
//   object          → recursive SchemaForm in an indented, titled sub-section
//   other/unknown   → muted "Unsupported type" notice
// Schema `title`/`description` become the label / help text; properties listed
// in `required` get a red `*` marker. Values are written into the shared
// `value` object, and `onChange` is called with the updated object whenever a
// field changes. Schema `default` values are pre-populated for keys that are
// not already set (via `collectMissingDefaults`).

import { useEffect, useId, useRef } from "react"

import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

export interface SchemaFormProps {
  /** JSON Schema object (with `properties` and optional `required`). */
  schema: Record<string, unknown>
  /** Current form values; the single source of truth for field states. */
  value: Record<string, unknown>
  /** Called with the updated values object whenever any field changes. */
  onChange: (value: Record<string, unknown>) => void
}

/**
 * Collect the schema `default` for every property that is missing from `value`
 * (i.e. `value[key] === undefined`). Only explicit `default` fields are
 * pre-populated — no zero values are invented for unset keys.
 */
export function collectMissingDefaults(
  schema: Record<string, unknown>,
  value: Record<string, unknown>
): Record<string, unknown> {
  const properties = asRecord(schema.properties)
  if (!properties) return {}

  const missing: Record<string, unknown> = {}
  for (const [key, prop] of Object.entries(properties)) {
    if (value[key] !== undefined) continue
    const propSchema = asRecord(prop)
    if (propSchema && "default" in propSchema) {
      missing[key] = propSchema.default
    }
  }
  return missing
}

export function SchemaForm({ schema, value, onChange }: SchemaFormProps) {
  const properties = asRecord(schema.properties)

  // Keys the user has interacted with. Defaults are only pre-populated for
  // untouched keys, so a user-cleared field stays cleared instead of having
  // its default silently re-applied.
  const touchedRef = useRef(new Set<string>())
  const prevSchemaRef = useRef(schema)

  // Pre-populate schema defaults for unset keys by writing back through
  // onChange. Converges: once the defaults are in `value` (or the key is
  // user-touched), nothing is missing.
  useEffect(() => {
    if (prevSchemaRef.current !== schema) {
      prevSchemaRef.current = schema
      touchedRef.current.clear()
    }
    const missing = collectMissingDefaults(schema, value)
    for (const key of Object.keys(missing)) {
      if (touchedRef.current.has(key)) delete missing[key]
    }
    const keys = Object.keys(missing)
    if (keys.length === 0) return
    onChange({ ...value, ...missing })
  }, [schema, value, onChange])

  if (!properties) {
    return <div className="text-xs text-muted-foreground">No parameters required</div>
  }

  const required = new Set(
    Array.isArray(schema.required)
      ? schema.required.filter((r): r is string => typeof r === "string")
      : []
  )

  const handleFieldChange = (name: string, next: Record<string, unknown>) => {
    touchedRef.current.add(name)
    onChange(next)
  }

  return (
    <div className="flex flex-col gap-3">
      {Object.entries(properties).map(([name, rawProp]) => (
        <SchemaProperty
          key={name}
          name={name}
          propSchema={asRecord(rawProp) ?? {}}
          required={required.has(name)}
          value={value}
          onFieldChange={handleFieldChange}
        />
      ))}
    </div>
  )
}

interface SchemaPropertyProps {
  name: string
  propSchema: Record<string, unknown>
  required: boolean
  value: Record<string, unknown>
  onFieldChange: (name: string, next: Record<string, unknown>) => void
}

function SchemaProperty({
  name,
  propSchema,
  required,
  value,
  onFieldChange,
}: SchemaPropertyProps) {
  const type = schemaType(propSchema)
  const label = typeof propSchema.title === "string" ? propSchema.title : name
  const description =
    typeof propSchema.description === "string" ? propSchema.description : undefined

  if (type === "object") {
    const subValue = (value[name] as Record<string, unknown> | undefined) ?? {}
    return (
      <div className="ml-2 rounded-md border border-border/70 p-3">
        <div className="mb-2 text-sm font-semibold">
          {label}
          {required && <span className="text-destructive"> *</span>}
        </div>
        <SchemaForm
          schema={propSchema}
          value={subValue}
          onChange={(sub) => onFieldChange(name, { ...value, [name]: sub })}
        />
        {description && (
          <p className="mt-1 text-xs text-muted-foreground">{description}</p>
        )}
      </div>
    )
  }

  if (type === "boolean") {
    const id = useSchemaFieldId(name)
    return (
      <div className="flex items-center gap-2">
        <Checkbox
          id={id}
          checked={value[name] === true}
          onCheckedChange={(next) =>
            onFieldChange(name, { ...value, [name]: next === true })
          }
        />
        <Label htmlFor={id} className="cursor-pointer">
          {label}
          {required && <span className="text-destructive"> *</span>}
        </Label>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
    )
  }

  if (type === "number" || type === "integer") {
    const current = value[name]
    const numText =
      typeof current === "number"
        ? String(current)
        : typeof current === "string"
          ? current
          : ""
    return (
      <div className="flex flex-col gap-1">
        <Label>
          {label}
          {required && <span className="text-destructive"> *</span>}
        </Label>
        <Input
          type="number"
          step={type === "integer" ? 1 : "any"}
          value={numText}
          onChange={(e) => {
            const raw = e.target.value
            if (raw === "") {
              const next = { ...value }
              delete next[name]
              onFieldChange(name, next)
            } else {
              const n = Number(raw)
              // Keep the raw string while mid-edit (e.g. "-" or "1e"); it
              // normalizes to a number on the next keystroke.
              onFieldChange(name, { ...value, [name]: Number.isNaN(n) ? raw : n })
            }
          }}
        />
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
    )
  }

  const enumValues = stringEnumValues(propSchema)
  if (enumValues.length > 0) {
    const current = value[name]
    const selected =
      typeof current === "string" && enumValues.includes(current)
        ? current
        : undefined
    return (
      <div className="flex flex-col gap-1">
        <Label>
          {label}
          {required && <span className="text-destructive"> *</span>}
        </Label>
        {/* Always controlled: "" shows the placeholder and avoids an
            uncontrolled → controlled transition when defaults hydrate. */}
        <Select
          value={selected ?? ""}
          onValueChange={(next) =>
            onFieldChange(name, { ...value, [name]: next })
          }
        >
          <SelectTrigger>
            <SelectValue placeholder="Select..." />
          </SelectTrigger>
          <SelectContent>
            {enumValues.map((opt) => (
              <SelectItem key={opt} value={opt}>
                {opt}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
    )
  }

  if (type === "string") {
    const current = value[name]
    const text = typeof current === "string" ? current : ""
    return (
      <div className="flex flex-col gap-1">
        <Label>
          {label}
          {required && <span className="text-destructive"> *</span>}
        </Label>
        <Input
          value={text}
          onChange={(e) =>
            onFieldChange(name, { ...value, [name]: e.target.value })
          }
        />
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </div>
    )
  }

  return (
    <div className="text-xs text-muted-foreground">Unsupported type: {type}</div>
  )
}

/** Unique id for a field, so checkbox labels stay associated across nesting. */
function useSchemaFieldId(name: string): string {
  const id = useId()
  return `schema-${id}-${name}`
}

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === "object" && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined
}

/** Schema `type` keyword; missing/non-string types default to "string". */
function schemaType(prop: Record<string, unknown>): string {
  return typeof prop.type === "string" ? prop.type : "string"
}

/**
 * The `enum` array when it is non-empty and contains only non-empty strings
 * (Radix Select item values must be non-empty strings); otherwise empty.
 */
function stringEnumValues(prop: Record<string, unknown>): string[] {
  const raw = prop.enum
  if (!Array.isArray(raw)) return []
  const strings = raw.filter(
    (v): v is string => typeof v === "string" && v.length > 0
  )
  return strings.length === raw.length ? strings : []
}
