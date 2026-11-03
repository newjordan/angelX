import Base
import ./field.bend as Field

law expired_field_is_quiet:
  for radius: U32
  {Field.tick(Field.Active{0n, radius}) == Field.Quiet{Field.Dormant{}} : Field.Step}

law dormant_field_is_quiet:
  {Field.tick(Field.Dormant{}) == Field.Quiet{Field.Dormant{}} : Field.Step}
