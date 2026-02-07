#!/usr/bin/env python3
"""
Generate synthetic GraphQL fixtures for benchmarking.

This script generates GraphQL operations and fragments of varying sizes
for performance testing the Relay compiler.
"""

import os

def generate_operation(i: int, prefix: str) -> str:
    """Generate a single GraphQL operation."""
    variant = i % 10

    if variant == 0:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{
      name
      firstName
      lastName
      profilePicture(size: 100) {{ uri width height }}
    }}
  }}
}}"""
    elif variant == 1:
        return f"""query {prefix}Query{i}($id: ID!, $first: Int!) {{
  node(id: $id) {{
    id
    ... on User {{
      name
      friends(first: $first) {{
        edges {{ node {{ id name }} }}
      }}
    }}
  }}
}}"""
    elif variant == 2:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{ name lastName email }}
  }}
}}"""
    elif variant == 3:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{
      birthdate {{ year month day }}
    }}
  }}
}}"""
    elif variant == 4:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{
      hometown {{ id name }}
    }}
  }}
}}"""
    elif variant == 5:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{ name }}
    ... on Page {{ name }}
  }}
}}"""
    elif variant == 6:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on Comment {{
      body {{ text }}
      author {{ name }}
    }}
  }}
}}"""
    elif variant == 7:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on Actor {{ name }}
  }}
}}"""
    elif variant == 8:
        return f"""query {prefix}Query{i}($id: ID!, $size: [Int]) {{
  node(id: $id) {{
    id
    ... on User {{
      profilePicture(size: $size) {{ uri }}
    }}
  }}
}}"""
    else:
        return f"""query {prefix}Query{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{
      screennames {{ name service }}
    }}
  }}
}}"""


def generate_fragment(i: int, prefix: str) -> str:
    """Generate a single GraphQL fragment."""
    variant = i % 8

    if variant == 0:
        return f"""fragment {prefix}Fragment{i} on User {{
  id
  name
  firstName
  lastName
}}"""
    elif variant == 1:
        return f"""fragment {prefix}Fragment{i} on User {{
  id
  profilePicture(size: 100) {{
    uri
    width
    height
  }}
}}"""
    elif variant == 2:
        return f"""fragment {prefix}Fragment{i} on User {{
  id
  email
  websites
}}"""
    elif variant == 3:
        return f"""fragment {prefix}Fragment{i} on User {{
  id
  birthdate {{
    year
    month
    day
  }}
}}"""
    elif variant == 4:
        return f"""fragment {prefix}Fragment{i} on User {{
  id
  hometown {{
    id
    name
  }}
}}"""
    elif variant == 5:
        return f"""fragment {prefix}Fragment{i} on Page {{
  id
  name
  likers {{ count }}
}}"""
    elif variant == 6:
        return f"""fragment {prefix}Fragment{i} on Comment {{
  id
  body {{ text }}
  author {{ name }}
}}"""
    else:
        return f"""fragment {prefix}Fragment{i} on Actor {{
  id
  name
}}"""


def generate_fixtures(num_operations: int, num_fragments: int, prefix: str, output_dir: str):
    """Generate a complete fixture file."""
    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, "operations.graphql")

    with open(output_path, "w") as f:
        f.write(f"# Benchmark fixtures - {num_operations} operations, {num_fragments} fragments\n")
        f.write("# Auto-generated for performance testing\n\n")

        # Generate fragments first (so operations can reference them)
        for i in range(num_fragments):
            f.write(generate_fragment(i, prefix))
            f.write("\n\n")

        # Generate operations
        for i in range(num_operations):
            f.write(generate_operation(i, prefix))
            f.write("\n\n")

    print(f"Generated {num_operations} operations and {num_fragments} fragments in {output_path}")


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))

    # Medium: 1000 operations, 200 fragments
    generate_fixtures(
        num_operations=800,
        num_fragments=200,
        prefix="Medium",
        output_dir=os.path.join(script_dir, "medium")
    )

    # Large: 10000 operations, 2000 fragments
    generate_fixtures(
        num_operations=8000,
        num_fragments=2000,
        prefix="Large",
        output_dir=os.path.join(script_dir, "large")
    )


if __name__ == "__main__":
    main()
