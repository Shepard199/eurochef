#include <fbxsdk.h>

#if FBXSDK_VERSION_MAJOR != 2020
#error "EuroChef FBX exporter requires Autodesk FBX SDK 2020.x"
#endif

#include <algorithm>
#include <array>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <vector>

namespace {

constexpr std::array<char, 8> kMagic{'E', 'C', 'F', 'B', 'X', '0', '0', '2'};
constexpr std::uint32_t kVersion = 2;
constexpr std::uint32_t kMaxBones = 4096;
constexpr std::uint32_t kMaxMeshes = 4096;
constexpr std::uint32_t kMaxClips = 100'000;
constexpr std::uint32_t kMaxFrames = 1'000'000;
constexpr std::uint32_t kMaxVertices = 20'000'000;
constexpr std::uint32_t kMaxIndices = 60'000'000;
constexpr std::uint32_t kMaxMaterials = 65'536;
constexpr std::uint32_t kMaxStringBytes = 1'048'576;
constexpr double kUnitScale = 100.0;
constexpr double kTimeTolerance = 1.0e-7;
constexpr double kTranslationToleranceCm = 1.0e-3;
constexpr double kRotationToleranceDegrees = 1.0e-2;
constexpr double kScaleTolerance = 1.0e-5;

struct Vec2 {
    float x{};
    float y{};
};

struct Vec3 {
    float x{};
    float y{};
    float z{};
};

struct Vec4 {
    float x{};
    float y{};
    float z{};
    float w{};
};

struct Quat {
    float x{};
    float y{};
    float z{};
    float w{1.0F};
};

struct Influence {
    std::array<std::uint16_t, 4> bones{};
    std::array<float, 4> weights{};
};

struct Vertex {
    Vec3 position;
    Vec3 normal;
    Vec2 uv;
    Vec4 color;
    Influence influence;
};

struct Bone {
    std::string name;
    std::int32_t parent{-1};
    Vec3 local_position;
    Vec3 global_position;
};

struct Material {
    std::uint32_t hashcode{};
    std::string name;
};

struct Mesh {
    std::string name;
    std::vector<Vertex> vertices;
    std::vector<std::uint32_t> indices;
    std::vector<std::uint32_t> triangle_materials;
    std::vector<Material> materials;
};

struct Pose {
    Vec3 position;
    Quat rotation;
    Vec3 scale;
};

struct AnimationClip {
    std::string name;
    std::uint32_t animation_uid{};
    std::uint32_t source_animation_index{};
    std::uint32_t source_script_uid{};
    std::uint32_t source_script_command{};
    std::uint32_t usage_count{};
    float source_script_fps{};
    std::uint32_t source_command_length{};
    float sample_rate{};
    std::uint32_t frame_count{};
    float duration_seconds{};
    std::string root_motion_mode;
    std::vector<Pose> poses;
};

struct Character {
    std::string name;
    std::uint32_t source_edb_uid{};
    std::uint32_t animskin_uid{};
    std::vector<Bone> bones;
    std::vector<Mesh> meshes;
    std::vector<AnimationClip> clips;
};

class BinaryReader {
public:
    explicit BinaryReader(const std::filesystem::path& path) : stream_(path, std::ios::binary) {
        if (!stream_) {
            throw std::runtime_error("cannot open IR file: " + path.string());
        }
    }

    template <typename T>
    T read() {
        static_assert(std::is_trivially_copyable_v<T>);
        T value{};
        stream_.read(reinterpret_cast<char*>(&value), sizeof(T));
        if (!stream_) {
            throw std::runtime_error("truncated IR file");
        }
        return value;
    }

    std::string read_string() {
        const auto size = read<std::uint32_t>();
        if (size > kMaxStringBytes) {
            throw std::runtime_error("IR string exceeds safety limit");
        }
        std::string value(size, '\0');
        if (size != 0) {
            stream_.read(value.data(), static_cast<std::streamsize>(size));
            if (!stream_) {
                throw std::runtime_error("truncated IR string");
            }
        }
        return value;
    }

    std::array<char, 8> read_magic() {
        std::array<char, 8> value{};
        stream_.read(value.data(), static_cast<std::streamsize>(value.size()));
        if (!stream_) {
            throw std::runtime_error("truncated IR header");
        }
        return value;
    }

private:
    std::ifstream stream_;
};

Vec2 read_vec2(BinaryReader& reader) {
    return {reader.read<float>(), reader.read<float>()};
}

Vec3 read_vec3(BinaryReader& reader) {
    return {reader.read<float>(), reader.read<float>(), reader.read<float>()};
}

Vec4 read_vec4(BinaryReader& reader) {
    return {reader.read<float>(), reader.read<float>(), reader.read<float>(), reader.read<float>()};
}

Quat read_quat(BinaryReader& reader) {
    return {reader.read<float>(), reader.read<float>(), reader.read<float>(), reader.read<float>()};
}

void ensure_finite(float value, const char* field) {
    if (!std::isfinite(value)) {
        throw std::runtime_error(std::string(field) + " contains a non-finite value");
    }
}

void validate_quaternion(const Quat& value, const char* field) {
    const double length = std::sqrt(
        static_cast<double>(value.x) * value.x + static_cast<double>(value.y) * value.y
        + static_cast<double>(value.z) * value.z + static_cast<double>(value.w) * value.w);
    if (!std::isfinite(length) || std::abs(length - 1.0) > 2.0e-3) {
        throw std::runtime_error(std::string(field) + " is not a unit quaternion");
    }
}

Character read_character(const std::filesystem::path& path) {
    BinaryReader reader(path);
    if (reader.read_magic() != kMagic) {
        throw std::runtime_error("unsupported EuroChef FBX IR magic");
    }
    if (reader.read<std::uint32_t>() != kVersion) {
        throw std::runtime_error("unsupported EuroChef FBX IR version");
    }

    Character character;
    character.name = reader.read_string();
    character.source_edb_uid = reader.read<std::uint32_t>();
    character.animskin_uid = reader.read<std::uint32_t>();

    const auto bone_count = reader.read<std::uint32_t>();
    if (bone_count == 0 || bone_count > kMaxBones) {
        throw std::runtime_error("invalid IR bone count");
    }
    character.bones.reserve(bone_count);
    for (std::uint32_t index = 0; index < bone_count; ++index) {
        Bone bone;
        bone.name = reader.read_string();
        bone.parent = reader.read<std::int32_t>();
        bone.local_position = read_vec3(reader);
        bone.global_position = read_vec3(reader);
        if (bone.name.empty()) {
            throw std::runtime_error("empty bone name");
        }
        if (bone.parent >= static_cast<std::int32_t>(index)) {
            throw std::runtime_error("bone parent must precede child");
        }
        for (float value : {bone.local_position.x, bone.local_position.y, bone.local_position.z,
                            bone.global_position.x, bone.global_position.y, bone.global_position.z}) {
            ensure_finite(value, "bone bind position");
        }
        character.bones.push_back(std::move(bone));
    }

    const auto mesh_count = reader.read<std::uint32_t>();
    if (mesh_count == 0 || mesh_count > kMaxMeshes) {
        throw std::runtime_error("invalid IR mesh count");
    }
    character.meshes.reserve(mesh_count);
    for (std::uint32_t mesh_index = 0; mesh_index < mesh_count; ++mesh_index) {
        Mesh mesh;
        mesh.name = reader.read_string();
        const auto vertex_count = reader.read<std::uint32_t>();
        if (vertex_count == 0 || vertex_count > kMaxVertices) {
            throw std::runtime_error("invalid IR vertex count");
        }
        mesh.vertices.reserve(vertex_count);
        for (std::uint32_t vertex_index = 0; vertex_index < vertex_count; ++vertex_index) {
            Vertex vertex;
            vertex.position = read_vec3(reader);
            vertex.normal = read_vec3(reader);
            vertex.uv = read_vec2(reader);
            vertex.color = read_vec4(reader);
            for (auto& bone : vertex.influence.bones) {
                bone = reader.read<std::uint16_t>();
                if (bone >= bone_count) {
                    throw std::runtime_error("skin influence references invalid bone");
                }
            }
            float weight_sum = 0.0F;
            for (auto& weight : vertex.influence.weights) {
                weight = reader.read<float>();
                ensure_finite(weight, "skin weight");
                if (weight < 0.0F) {
                    throw std::runtime_error("negative skin weight");
                }
                weight_sum += weight;
            }
            if (std::abs(weight_sum - 1.0F) > 1.0e-4F) {
                throw std::runtime_error("skin weights do not sum to one");
            }
            for (float value : {vertex.position.x, vertex.position.y, vertex.position.z,
                                vertex.normal.x, vertex.normal.y, vertex.normal.z,
                                vertex.uv.x, vertex.uv.y, vertex.color.x, vertex.color.y,
                                vertex.color.z, vertex.color.w}) {
                ensure_finite(value, "vertex");
            }
            mesh.vertices.push_back(vertex);
        }

        const auto index_count = reader.read<std::uint32_t>();
        if (index_count == 0 || index_count > kMaxIndices || index_count % 3 != 0) {
            throw std::runtime_error("invalid IR triangle index count");
        }
        mesh.indices.reserve(index_count);
        for (std::uint32_t index = 0; index < index_count; ++index) {
            const auto value = reader.read<std::uint32_t>();
            if (value >= vertex_count) {
                throw std::runtime_error("triangle index outside vertex array");
            }
            mesh.indices.push_back(value);
        }

        const auto triangle_count = reader.read<std::uint32_t>();
        if (triangle_count != index_count / 3) {
            throw std::runtime_error("IR triangle/material count mismatch");
        }
        mesh.triangle_materials.reserve(triangle_count);
        for (std::uint32_t triangle = 0; triangle < triangle_count; ++triangle) {
            mesh.triangle_materials.push_back(reader.read<std::uint32_t>());
        }

        const auto material_count = reader.read<std::uint32_t>();
        if (material_count == 0 || material_count > kMaxMaterials) {
            throw std::runtime_error("invalid IR material count");
        }
        mesh.materials.reserve(material_count);
        for (std::uint32_t material = 0; material < material_count; ++material) {
            mesh.materials.push_back({reader.read<std::uint32_t>(), reader.read_string()});
        }
        for (const auto slot : mesh.triangle_materials) {
            if (slot >= material_count) {
                throw std::runtime_error("triangle references invalid material slot");
            }
        }
        character.meshes.push_back(std::move(mesh));
    }

    const auto clip_count = reader.read<std::uint32_t>();
    if (clip_count > kMaxClips) {
        throw std::runtime_error("invalid IR animation clip count");
    }
    character.clips.reserve(clip_count);
    for (std::uint32_t clip_index = 0; clip_index < clip_count; ++clip_index) {
        AnimationClip clip;
        clip.name = reader.read_string();
        clip.animation_uid = reader.read<std::uint32_t>();
        clip.source_animation_index = reader.read<std::uint32_t>();
        clip.source_script_uid = reader.read<std::uint32_t>();
        clip.source_script_command = reader.read<std::uint32_t>();
        clip.usage_count = reader.read<std::uint32_t>();
        clip.source_script_fps = reader.read<float>();
        clip.source_command_length = reader.read<std::uint32_t>();
        clip.sample_rate = reader.read<float>();
        clip.frame_count = reader.read<std::uint32_t>();
        clip.duration_seconds = reader.read<float>();
        clip.root_motion_mode = reader.read_string();
        const auto pose_count = reader.read<std::uint32_t>();
        const std::uint64_t expected_pose_count =
            static_cast<std::uint64_t>(clip.frame_count) * bone_count;
        if (clip.name.empty() || clip.frame_count == 0 || clip.frame_count > kMaxFrames
            || !std::isfinite(clip.sample_rate) || clip.sample_rate <= 0.0F
            || !std::isfinite(clip.duration_seconds) || clip.duration_seconds < 0.0F
            || expected_pose_count > std::numeric_limits<std::uint32_t>::max()
            || pose_count != expected_pose_count) {
            throw std::runtime_error("invalid IR animation clip metadata or pose dimensions");
        }
        if (clip.source_command_length != 0
            && (!std::isfinite(clip.source_script_fps) || clip.source_script_fps <= 0.0F)) {
            throw std::runtime_error("animation has invalid serialized Script FPS");
        }
        const double expected_duration = clip.source_command_length != 0
            ? static_cast<double>(clip.source_command_length) / clip.source_script_fps
            : (clip.frame_count > 1
                   ? static_cast<double>(clip.frame_count - 1) / clip.sample_rate
                   : 0.0);
        if (std::abs(expected_duration - clip.duration_seconds) > 1.0e-5) {
            throw std::runtime_error("animation duration does not match serialized timing");
        }
        clip.poses.reserve(pose_count);
        for (std::uint32_t pose_index = 0; pose_index < pose_count; ++pose_index) {
            Pose pose;
            pose.position = read_vec3(reader);
            pose.rotation = read_quat(reader);
            pose.scale = read_vec3(reader);
            for (float value : {pose.position.x, pose.position.y, pose.position.z,
                                pose.rotation.x, pose.rotation.y, pose.rotation.z, pose.rotation.w,
                                pose.scale.x, pose.scale.y, pose.scale.z}) {
                ensure_finite(value, "animation pose");
            }
            validate_quaternion(pose.rotation, "animation rotation");
            if (pose.scale.x <= 0.0F || pose.scale.y <= 0.0F || pose.scale.z <= 0.0F) {
                throw std::runtime_error("animation scale must be positive");
            }
            clip.poses.push_back(pose);
        }
        character.clips.push_back(std::move(clip));
    }
    return character;
}

FbxVector4 convert_point(const Vec3& value) {
    return FbxVector4(-value.x * kUnitScale, -value.z * kUnitScale, value.y * kUnitScale, 1.0);
}

FbxVector4 convert_normal(const Vec3& value) {
    FbxVector4 result(-value.x, -value.z, value.y, 0.0);
    result.Normalize();
    return result;
}

FbxDouble3 convert_scale(const Vec3& value) {
    return FbxDouble3(value.x, value.z, value.y);
}

using Mat3 = std::array<std::array<double, 3>, 3>;

Mat3 multiply(const Mat3& left, const Mat3& right) {
    Mat3 output{};
    for (std::size_t row = 0; row < 3; ++row) {
        for (std::size_t column = 0; column < 3; ++column) {
            for (std::size_t inner = 0; inner < 3; ++inner) {
                output[row][column] += left[row][inner] * right[inner][column];
            }
        }
    }
    return output;
}

Mat3 transpose(const Mat3& value) {
    Mat3 output{};
    for (std::size_t row = 0; row < 3; ++row) {
        for (std::size_t column = 0; column < 3; ++column) {
            output[row][column] = value[column][row];
        }
    }
    return output;
}

Mat3 quaternion_matrix(const Quat& value) {
    const double x = value.x;
    const double y = value.y;
    const double z = value.z;
    const double w = value.w;
    return {{{1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w),
              2.0 * (x * z + y * w)},
             {2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z),
              2.0 * (y * z - x * w)},
             {2.0 * (x * z - y * w), 2.0 * (y * z + x * w),
              1.0 - 2.0 * (x * x + y * y)}}};
}

Quat matrix_quaternion(const Mat3& matrix) {
    Quat result;
    const double trace = matrix[0][0] + matrix[1][1] + matrix[2][2];
    if (trace > 0.0) {
        const double s = std::sqrt(trace + 1.0) * 2.0;
        result.w = static_cast<float>(0.25 * s);
        result.x = static_cast<float>((matrix[2][1] - matrix[1][2]) / s);
        result.y = static_cast<float>((matrix[0][2] - matrix[2][0]) / s);
        result.z = static_cast<float>((matrix[1][0] - matrix[0][1]) / s);
    } else if (matrix[0][0] > matrix[1][1] && matrix[0][0] > matrix[2][2]) {
        const double s = std::sqrt(1.0 + matrix[0][0] - matrix[1][1] - matrix[2][2]) * 2.0;
        result.w = static_cast<float>((matrix[2][1] - matrix[1][2]) / s);
        result.x = static_cast<float>(0.25 * s);
        result.y = static_cast<float>((matrix[0][1] + matrix[1][0]) / s);
        result.z = static_cast<float>((matrix[0][2] + matrix[2][0]) / s);
    } else if (matrix[1][1] > matrix[2][2]) {
        const double s = std::sqrt(1.0 + matrix[1][1] - matrix[0][0] - matrix[2][2]) * 2.0;
        result.w = static_cast<float>((matrix[0][2] - matrix[2][0]) / s);
        result.x = static_cast<float>((matrix[0][1] + matrix[1][0]) / s);
        result.y = static_cast<float>(0.25 * s);
        result.z = static_cast<float>((matrix[1][2] + matrix[2][1]) / s);
    } else {
        const double s = std::sqrt(1.0 + matrix[2][2] - matrix[0][0] - matrix[1][1]) * 2.0;
        result.w = static_cast<float>((matrix[1][0] - matrix[0][1]) / s);
        result.x = static_cast<float>((matrix[0][2] + matrix[2][0]) / s);
        result.y = static_cast<float>((matrix[1][2] + matrix[2][1]) / s);
        result.z = static_cast<float>(0.25 * s);
    }
    const double length = std::sqrt(
        static_cast<double>(result.x) * result.x + static_cast<double>(result.y) * result.y
        + static_cast<double>(result.z) * result.z + static_cast<double>(result.w) * result.w);
    result.x = static_cast<float>(result.x / length);
    result.y = static_cast<float>(result.y / length);
    result.z = static_cast<float>(result.z / length);
    result.w = static_cast<float>(result.w / length);
    return result;
}

Quat convert_rotation(const Quat& source) {
    // target = B * source * B^-1 where B maps source vectors to (-x, -z, y).
    const Mat3 basis{{{-1.0, 0.0, 0.0}, {0.0, 0.0, -1.0}, {0.0, 1.0, 0.0}}};
    return matrix_quaternion(multiply(multiply(basis, quaternion_matrix(source)), transpose(basis)));
}

FbxVector4 rotation_euler_xyz(const Quat& source) {
    const Quat converted = convert_rotation(source);
    FbxAMatrix matrix;
    matrix.SetIdentity();
    matrix.SetQ(FbxQuaternion(converted.x, converted.y, converted.z, converted.w));
    return matrix.GetR();
}

double unwrap_angle(double current, double previous) {
    while (current - previous > 180.0) {
        current -= 360.0;
    }
    while (current - previous < -180.0) {
        current += 360.0;
    }
    return current;
}

std::vector<FbxVector4> build_unwrapped_euler_tracks(
    const AnimationClip& clip,
    std::size_t bone_count) {
    std::vector<FbxVector4> tracks(clip.poses.size());
    for (std::size_t bone_index = 0; bone_index < bone_count; ++bone_index) {
        FbxVector4 previous;
        for (std::size_t frame = 0; frame < clip.frame_count; ++frame) {
            const std::size_t pose_index = frame * bone_count + bone_index;
            FbxVector4 current = rotation_euler_xyz(clip.poses[pose_index].rotation);
            if (frame != 0) {
                current[0] = unwrap_angle(current[0], previous[0]);
                current[1] = unwrap_angle(current[1], previous[1]);
                current[2] = unwrap_angle(current[2], previous[2]);
            }
            tracks[pose_index] = current;
            previous = current;
        }
    }
    return tracks;
}

FbxAMatrix translation_matrix(const Vec3& position) {
    FbxAMatrix matrix;
    matrix.SetIdentity();
    matrix.SetT(convert_point(position));
    return matrix;
}

std::string lower_copy(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return value;
}

int binary_writer_format(FbxManager* manager) {
    FbxIOPluginRegistry* registry = manager->GetIOPluginRegistry();
    const int count = registry->GetWriterFormatCount();
    for (int index = 0; index < count; ++index) {
        if (!registry->WriterIsFBX(index)) {
            continue;
        }
        const char* description = registry->GetWriterFormatDescription(index);
        if (description && lower_copy(description).find("binary") != std::string::npos) {
            return index;
        }
    }
    return registry->GetNativeWriterFormat();
}

struct SkeletonScene {
    std::vector<FbxNode*> nodes;
    std::vector<FbxAMatrix> bind_matrices;
};

SkeletonScene build_skeleton(FbxScene* scene, const Character& character) {
    SkeletonScene skeleton;
    skeleton.nodes.resize(character.bones.size(), nullptr);
    skeleton.bind_matrices.resize(character.bones.size());
    FbxNode* scene_root = scene->GetRootNode();
    for (std::size_t index = 0; index < character.bones.size(); ++index) {
        const Bone& bone = character.bones[index];
        FbxSkeleton* attribute = FbxSkeleton::Create(scene, bone.name.c_str());
        attribute->SetSkeletonType(bone.parent < 0 ? FbxSkeleton::eRoot : FbxSkeleton::eLimbNode);
        attribute->Size.Set(1.0);

        FbxNode* node = FbxNode::Create(scene, bone.name.c_str());
        node->SetNodeAttribute(attribute);
        node->SetRotationActive(true);
        node->SetRotationOrder(FbxNode::eSourcePivot, FbxEuler::eOrderXYZ);
        const FbxVector4 local = convert_point(bone.local_position);
        node->LclTranslation.Set(FbxDouble3(local[0], local[1], local[2]));
        node->LclRotation.Set(FbxDouble3(0.0, 0.0, 0.0));
        node->LclScaling.Set(FbxDouble3(1.0, 1.0, 1.0));
        if (bone.parent < 0) {
            scene_root->AddChild(node);
        } else {
            skeleton.nodes.at(static_cast<std::size_t>(bone.parent))->AddChild(node);
        }
        skeleton.nodes[index] = node;
        skeleton.bind_matrices[index] = translation_matrix(bone.global_position);
    }
    return skeleton;
}

FbxPose* add_bind_pose(
    FbxScene* scene,
    const Character& character,
    const SkeletonScene& skeleton,
    const std::vector<FbxNode*>& mesh_nodes) {
    FbxPose* bind_pose = FbxPose::Create(scene, (character.name + "_BindPose").c_str());
    bind_pose->SetIsBindPose(true);
    FbxAMatrix identity;
    identity.SetIdentity();
    for (FbxNode* mesh_node : mesh_nodes) {
        bind_pose->Add(mesh_node, identity);
    }
    for (std::size_t bone_index = 0; bone_index < skeleton.nodes.size(); ++bone_index) {
        bind_pose->Add(skeleton.nodes[bone_index], skeleton.bind_matrices[bone_index]);
    }
    scene->AddPose(bind_pose);
    return bind_pose;
}

struct ModelBuildResult {
    std::uint64_t triangle_count{};
    std::uint64_t vertex_count{};
    std::uint64_t cluster_count{};
};

ModelBuildResult build_model_scene(FbxScene* scene, const Character& character) {
    scene->GetGlobalSettings().SetAxisSystem(FbxAxisSystem::MayaZUp);
    scene->GetGlobalSettings().SetSystemUnit(FbxSystemUnit::cm);
    const SkeletonScene skeleton = build_skeleton(scene, character);
    FbxNode* scene_root = scene->GetRootNode();

    ModelBuildResult result;
    std::vector<FbxNode*> mesh_nodes;
    mesh_nodes.reserve(character.meshes.size());
    for (const Mesh& source : character.meshes) {
        FbxMesh* mesh = FbxMesh::Create(scene, source.name.c_str());
        FbxNode* mesh_node = FbxNode::Create(scene, source.name.c_str());
        mesh_node->SetNodeAttribute(mesh);
        scene_root->AddChild(mesh_node);
        mesh_nodes.push_back(mesh_node);

        mesh->InitControlPoints(static_cast<int>(source.vertices.size()));
        FbxVector4* control_points = mesh->GetControlPoints();
        for (std::size_t index = 0; index < source.vertices.size(); ++index) {
            control_points[index] = convert_point(source.vertices[index].position);
        }

        FbxGeometryElementNormal* normals = mesh->CreateElementNormal();
        normals->SetMappingMode(FbxGeometryElement::eByPolygonVertex);
        normals->SetReferenceMode(FbxGeometryElement::eDirect);
        FbxGeometryElementUV* uvs = mesh->CreateElementUV("UVChannel_0");
        uvs->SetMappingMode(FbxGeometryElement::eByPolygonVertex);
        uvs->SetReferenceMode(FbxGeometryElement::eDirect);
        FbxGeometryElementVertexColor* colors = mesh->CreateElementVertexColor();
        colors->SetMappingMode(FbxGeometryElement::eByPolygonVertex);
        colors->SetReferenceMode(FbxGeometryElement::eDirect);
        FbxGeometryElementMaterial* material_element = mesh->CreateElementMaterial();
        material_element->SetMappingMode(FbxGeometryElement::eByPolygon);
        material_element->SetReferenceMode(FbxGeometryElement::eIndexToDirect);

        for (const Material& material : source.materials) {
            FbxSurfacePhong* phong = FbxSurfacePhong::Create(scene, material.name.c_str());
            phong->Diffuse.Set(FbxDouble3(1.0, 1.0, 1.0));
            phong->DiffuseFactor.Set(1.0);
            phong->ShadingModel.Set("Phong");
            mesh_node->AddMaterial(phong);
        }

        const std::size_t triangle_count = source.indices.size() / 3;
        for (std::size_t triangle = 0; triangle < triangle_count; ++triangle) {
            const std::uint32_t material_slot = source.triangle_materials[triangle];
            material_element->GetIndexArray().Add(static_cast<int>(material_slot));
            mesh->BeginPolygon(-1, -1, -1, false);
            const std::array<std::size_t, 3> order{0, 2, 1};
            for (const std::size_t corner : order) {
                const std::uint32_t control_point = source.indices[triangle * 3 + corner];
                const Vertex& vertex = source.vertices[control_point];
                mesh->AddPolygon(static_cast<int>(control_point));
                normals->GetDirectArray().Add(convert_normal(vertex.normal));
                uvs->GetDirectArray().Add(FbxVector2(vertex.uv.x, vertex.uv.y));
                colors->GetDirectArray().Add(
                    FbxColor(vertex.color.x, vertex.color.y, vertex.color.z, vertex.color.w));
            }
            mesh->EndPolygon();
        }

        FbxSkin* skin = FbxSkin::Create(scene, (source.name + "_Skin").c_str());
        std::vector<FbxCluster*> clusters(character.bones.size(), nullptr);
        for (std::size_t bone_index = 0; bone_index < character.bones.size(); ++bone_index) {
            FbxCluster* cluster = FbxCluster::Create(
                scene, (source.name + "_" + character.bones[bone_index].name + "_Cluster").c_str());
            cluster->SetLink(skeleton.nodes[bone_index]);
            cluster->SetLinkMode(FbxCluster::eNormalize);
            clusters[bone_index] = cluster;
        }

        std::vector<double> accumulated_weights(character.bones.size(), 0.0);
        std::vector<std::uint16_t> touched_bones;
        touched_bones.reserve(4);
        for (std::size_t vertex_index = 0; vertex_index < source.vertices.size(); ++vertex_index) {
            const Influence& influence = source.vertices[vertex_index].influence;
            touched_bones.clear();
            for (std::size_t lane = 0; lane < influence.weights.size(); ++lane) {
                const double weight = influence.weights[lane];
                if (weight <= std::numeric_limits<double>::epsilon()) {
                    continue;
                }
                const std::uint16_t bone_index = influence.bones[lane];
                if (accumulated_weights[bone_index] == 0.0) {
                    touched_bones.push_back(bone_index);
                }
                accumulated_weights[bone_index] += weight;
            }
            for (const std::uint16_t bone_index : touched_bones) {
                clusters.at(bone_index)->AddControlPointIndex(
                    static_cast<int>(vertex_index), accumulated_weights[bone_index]);
                accumulated_weights[bone_index] = 0.0;
            }
        }

        FbxAMatrix mesh_bind;
        mesh_bind.SetIdentity();
        for (std::size_t bone_index = 0; bone_index < clusters.size(); ++bone_index) {
            FbxCluster* cluster = clusters[bone_index];
            if (cluster->GetControlPointIndicesCount() == 0) {
                cluster->Destroy();
                continue;
            }
            cluster->SetTransformMatrix(mesh_bind);
            cluster->SetTransformLinkMatrix(skeleton.bind_matrices[bone_index]);
            skin->AddCluster(cluster);
            ++result.cluster_count;
        }
        if (skin->GetClusterCount() == 0) {
            throw std::runtime_error("mesh contains no non-zero skin clusters");
        }
        mesh->AddDeformer(skin);
        result.triangle_count += triangle_count;
        result.vertex_count += source.vertices.size();
    }
    add_bind_pose(scene, character, skeleton, mesh_nodes);
    return result;
}

void write_curve_keys(
    FbxAnimCurve* curve,
    const std::vector<double>& values,
    double duration_seconds) {
    if (!curve || values.empty()) {
        throw std::runtime_error("animation curve or key payload is empty");
    }
    curve->KeyModifyBegin();
    for (std::size_t frame = 0; frame < values.size(); ++frame) {
        FbxTime time;
        const double seconds = values.size() > 1
            ? duration_seconds * static_cast<double>(frame) / static_cast<double>(values.size() - 1)
            : 0.0;
        time.SetSecondDouble(seconds);
        const int key_index = curve->KeyAdd(time);
        curve->KeySet(
            key_index,
            time,
            static_cast<float>(values[frame]),
            FbxAnimCurveDef::eInterpolationLinear);
    }
    curve->KeyModifyEnd();
}

struct AnimationBuildResult {
    std::uint64_t curve_count{};
    std::uint64_t key_count{};
};

AnimationBuildResult build_animation_scene(
    FbxScene* scene,
    const Character& character,
    const AnimationClip& clip) {
    FbxTime::SetGlobalTimeMode(FbxTime::eCustom, clip.sample_rate);
    scene->GetGlobalSettings().SetAxisSystem(FbxAxisSystem::MayaZUp);
    scene->GetGlobalSettings().SetSystemUnit(FbxSystemUnit::cm);
    scene->GetGlobalSettings().SetTimeMode(FbxTime::eCustom);
    scene->GetGlobalSettings().SetCustomFrameRate(clip.sample_rate);
    const SkeletonScene skeleton = build_skeleton(scene, character);
    add_bind_pose(scene, character, skeleton, {});

    FbxAnimStack* stack = FbxAnimStack::Create(scene, clip.name.c_str());
    FbxAnimLayer* layer = FbxAnimLayer::Create(scene, (clip.name + "_BaseLayer").c_str());
    stack->AddMember(layer);
    FbxTime start;
    start.SetSecondDouble(0.0);
    FbxTime stop;
    stop.SetSecondDouble(clip.duration_seconds);
    FbxTimeSpan span(start, stop);
    stack->SetLocalTimeSpan(span);
    scene->GetGlobalSettings().SetTimelineDefaultTimeSpan(span);

    const std::size_t bone_count = character.bones.size();
    const std::vector<FbxVector4> euler_tracks = build_unwrapped_euler_tracks(clip, bone_count);
    AnimationBuildResult result;
    for (std::size_t bone_index = 0; bone_index < bone_count; ++bone_index) {
        std::array<std::vector<double>, 9> channels;
        for (auto& channel : channels) {
            channel.reserve(clip.frame_count);
        }
        for (std::size_t frame = 0; frame < clip.frame_count; ++frame) {
            const std::size_t pose_index = frame * bone_count + bone_index;
            const Pose& pose = clip.poses[pose_index];
            const FbxVector4 translation = convert_point(pose.position);
            const FbxVector4 rotation = euler_tracks[pose_index];
            const FbxDouble3 scale = convert_scale(pose.scale);
            channels[0].push_back(translation[0]);
            channels[1].push_back(translation[1]);
            channels[2].push_back(translation[2]);
            channels[3].push_back(rotation[0]);
            channels[4].push_back(rotation[1]);
            channels[5].push_back(rotation[2]);
            channels[6].push_back(scale[0]);
            channels[7].push_back(scale[1]);
            channels[8].push_back(scale[2]);
        }

        FbxNode* node = skeleton.nodes[bone_index];
        std::array<FbxAnimCurve*, 9> curves{
            node->LclTranslation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_X, true),
            node->LclTranslation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Y, true),
            node->LclTranslation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Z, true),
            node->LclRotation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_X, true),
            node->LclRotation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Y, true),
            node->LclRotation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Z, true),
            node->LclScaling.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_X, true),
            node->LclScaling.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Y, true),
            node->LclScaling.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Z, true),
        };
        for (std::size_t channel = 0; channel < curves.size(); ++channel) {
            write_curve_keys(curves[channel], channels[channel], clip.duration_seconds);
            ++result.curve_count;
            result.key_count += channels[channel].size();
        }
    }
    return result;
}

void configure_export_io(FbxManager* manager, bool animation_only) {
    FbxIOSettings* settings = FbxIOSettings::Create(manager, IOSROOT);
    manager->SetIOSettings(settings);
    settings->SetBoolProp(EXP_FBX_MATERIAL, !animation_only);
    settings->SetBoolProp(EXP_FBX_TEXTURE, false);
    settings->SetBoolProp(EXP_FBX_EMBEDDED, false);
    settings->SetBoolProp(EXP_FBX_SHAPE, !animation_only);
    settings->SetBoolProp(EXP_FBX_GOBO, false);
    settings->SetBoolProp(EXP_FBX_ANIMATION, animation_only);
    settings->SetBoolProp(EXP_FBX_GLOBAL_SETTINGS, true);
}

struct ModelRoundTripResult {
    std::uint64_t triangle_count{};
    std::uint64_t vertex_count{};
    std::uint64_t bone_count{};
    std::uint64_t cluster_count{};
    std::uint64_t bind_pose_count{};
};

void inspect_model_node(FbxNode* node, ModelRoundTripResult& result) {
    if (!node) {
        return;
    }
    if (FbxNodeAttribute* attribute = node->GetNodeAttribute()) {
        if (attribute->GetAttributeType() == FbxNodeAttribute::eSkeleton) {
            ++result.bone_count;
        } else if (attribute->GetAttributeType() == FbxNodeAttribute::eMesh) {
            auto* mesh = static_cast<FbxMesh*>(attribute);
            result.triangle_count += static_cast<std::uint64_t>(mesh->GetPolygonCount());
            result.vertex_count += static_cast<std::uint64_t>(mesh->GetControlPointsCount());
            const int skin_count = mesh->GetDeformerCount(FbxDeformer::eSkin);
            for (int skin_index = 0; skin_index < skin_count; ++skin_index) {
                auto* skin = static_cast<FbxSkin*>(mesh->GetDeformer(skin_index, FbxDeformer::eSkin));
                result.cluster_count += static_cast<std::uint64_t>(skin->GetClusterCount());
            }
        }
    }
    for (int child = 0; child < node->GetChildCount(); ++child) {
        inspect_model_node(node->GetChild(child), result);
    }
}

FbxScene* import_scene(FbxManager* manager, const std::filesystem::path& path) {
    FbxImporter* importer = FbxImporter::Create(manager, "EuroChefRoundTripImporter");
    if (!importer->Initialize(path.string().c_str(), -1, manager->GetIOSettings())) {
        const std::string error = importer->GetStatus().GetErrorString();
        importer->Destroy();
        throw std::runtime_error("FBX round-trip importer initialization failed: " + error);
    }
    FbxScene* imported = FbxScene::Create(manager, "EuroChefRoundTripScene");
    if (!importer->Import(imported)) {
        const std::string error = importer->GetStatus().GetErrorString();
        importer->Destroy();
        imported->Destroy();
        throw std::runtime_error("FBX round-trip import failed: " + error);
    }
    importer->Destroy();
    return imported;
}

std::uint64_t bind_pose_count(FbxScene* scene) {
    std::uint64_t count = 0;
    for (int pose_index = 0; pose_index < scene->GetPoseCount(); ++pose_index) {
        if (scene->GetPose(pose_index)->IsBindPose()) {
            ++count;
        }
    }
    return count;
}

ModelRoundTripResult validate_model_round_trip(
    FbxManager* manager,
    const std::filesystem::path& path) {
    FbxScene* imported = import_scene(manager, path);
    ModelRoundTripResult result;
    inspect_model_node(imported->GetRootNode(), result);
    result.bind_pose_count = bind_pose_count(imported);
    imported->Destroy();
    return result;
}

double angle_difference(double left, double right) {
    double difference = std::fmod(left - right, 360.0);
    if (difference > 180.0) {
        difference -= 360.0;
    } else if (difference < -180.0) {
        difference += 360.0;
    }
    return std::abs(difference);
}

void validate_curve(
    FbxAnimCurve* curve,
    const std::vector<double>& expected,
    double duration_seconds,
    double value_tolerance,
    bool angular) {
    if (!curve || curve->KeyGetCount() != static_cast<int>(expected.size())) {
        throw std::runtime_error("animation round-trip curve key count mismatch");
    }
    for (std::size_t index : {std::size_t{0}, expected.size() - 1}) {
        const double expected_time = expected.size() > 1
            ? duration_seconds * static_cast<double>(index) / static_cast<double>(expected.size() - 1)
            : 0.0;
        const double actual_time = curve->KeyGetTime(static_cast<int>(index)).GetSecondDouble();
        const double actual_value = curve->KeyGetValue(static_cast<int>(index));
        const double error = angular
            ? angle_difference(actual_value, expected[index])
            : std::abs(actual_value - expected[index]);
        if (std::abs(actual_time - expected_time) > kTimeTolerance || error > value_tolerance) {
            throw std::runtime_error("animation round-trip first/last key mismatch");
        }
    }
}

struct AnimationRoundTripResult {
    std::uint64_t bone_count{};
    std::uint64_t mesh_count{};
    std::uint64_t stack_count{};
    std::uint64_t layer_count{};
    std::uint64_t curve_count{};
    std::uint64_t key_count{};
    std::uint64_t bind_pose_count{};
    bool custom_time_mode{};
    double custom_frame_rate{};
    double start_seconds{};
    double stop_seconds{};
};

AnimationRoundTripResult validate_animation_round_trip(
    FbxManager* manager,
    const std::filesystem::path& path,
    const Character& character,
    const AnimationClip& clip) {
    FbxScene* imported = import_scene(manager, path);
    AnimationRoundTripResult result;
    result.bind_pose_count = bind_pose_count(imported);
    result.mesh_count = imported->GetSrcObjectCount<FbxMesh>();
    result.custom_time_mode = imported->GetGlobalSettings().GetTimeMode() == FbxTime::eCustom;
    result.custom_frame_rate = imported->GetGlobalSettings().GetCustomFrameRate();
    result.stack_count = imported->GetSrcObjectCount<FbxAnimStack>();
    if (result.stack_count != 1) {
        imported->Destroy();
        throw std::runtime_error("animation round-trip must contain exactly one AnimStack");
    }
    FbxAnimStack* stack = imported->GetSrcObject<FbxAnimStack>(0);
    result.layer_count = stack->GetMemberCount<FbxAnimLayer>();
    if (result.layer_count != 1) {
        imported->Destroy();
        throw std::runtime_error("animation round-trip must contain exactly one AnimLayer");
    }
    FbxAnimLayer* layer = stack->GetMember<FbxAnimLayer>(0);
    FbxTimeSpan span = stack->GetLocalTimeSpan();
    result.start_seconds = span.GetStart().GetSecondDouble();
    result.stop_seconds = span.GetStop().GetSecondDouble();
    const std::vector<FbxVector4> euler_tracks =
        build_unwrapped_euler_tracks(clip, character.bones.size());

    for (std::size_t bone_index = 0; bone_index < character.bones.size(); ++bone_index) {
        FbxNode* node = imported->GetRootNode()->FindChild(character.bones[bone_index].name.c_str(), true);
        if (!node || !node->GetNodeAttribute()
            || node->GetNodeAttribute()->GetAttributeType() != FbxNodeAttribute::eSkeleton) {
            imported->Destroy();
            throw std::runtime_error("animation round-trip skeleton hierarchy is incomplete");
        }
        const Bone& bone = character.bones[bone_index];
        FbxNode* expected_parent = bone.parent < 0
            ? imported->GetRootNode()
            : imported->GetRootNode()->FindChild(
                  character.bones[static_cast<std::size_t>(bone.parent)].name.c_str(), true);
        if (!expected_parent || node->GetParent() != expected_parent) {
            imported->Destroy();
            throw std::runtime_error("animation round-trip bone parent mismatch");
        }
        const FbxVector4 expected_bind_translation = convert_point(bone.local_position);
        const FbxDouble3 actual_bind_translation = node->LclTranslation.Get();
        for (int axis = 0; axis < 3; ++axis) {
            if (std::abs(actual_bind_translation[axis] - expected_bind_translation[axis])
                > kTranslationToleranceCm) {
                imported->Destroy();
                throw std::runtime_error("animation round-trip reference pose mismatch");
            }
        }
        ++result.bone_count;
        std::array<std::vector<double>, 9> expected;
        for (auto& channel : expected) {
            channel.reserve(clip.frame_count);
        }
        for (std::size_t frame = 0; frame < clip.frame_count; ++frame) {
            const std::size_t pose_index = frame * character.bones.size() + bone_index;
            const Pose& pose = clip.poses[pose_index];
            const FbxVector4 translation = convert_point(pose.position);
            const FbxVector4 rotation = euler_tracks[pose_index];
            const FbxDouble3 scale = convert_scale(pose.scale);
            expected[0].push_back(translation[0]);
            expected[1].push_back(translation[1]);
            expected[2].push_back(translation[2]);
            expected[3].push_back(rotation[0]);
            expected[4].push_back(rotation[1]);
            expected[5].push_back(rotation[2]);
            expected[6].push_back(scale[0]);
            expected[7].push_back(scale[1]);
            expected[8].push_back(scale[2]);
        }
        std::array<FbxAnimCurve*, 9> curves{
            node->LclTranslation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_X, false),
            node->LclTranslation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Y, false),
            node->LclTranslation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Z, false),
            node->LclRotation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_X, false),
            node->LclRotation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Y, false),
            node->LclRotation.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Z, false),
            node->LclScaling.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_X, false),
            node->LclScaling.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Y, false),
            node->LclScaling.GetCurve(layer, FBXSDK_CURVENODE_COMPONENT_Z, false),
        };
        for (std::size_t channel = 0; channel < curves.size(); ++channel) {
            const bool angular = channel >= 3 && channel <= 5;
            const double tolerance = channel <= 2
                ? kTranslationToleranceCm
                : (angular ? kRotationToleranceDegrees : kScaleTolerance);
            validate_curve(
                curves[channel], expected[channel], clip.duration_seconds, tolerance, angular);
            ++result.curve_count;
            result.key_count += curves[channel]->KeyGetCount();
        }
    }

    if (result.bone_count != character.bones.size()
        || result.mesh_count != 0
        || result.bind_pose_count == 0
        || !result.custom_time_mode
        || std::abs(result.custom_frame_rate - clip.sample_rate) > 1.0e-6
        || std::abs(result.start_seconds) > kTimeTolerance
        || std::abs(result.stop_seconds - clip.duration_seconds) > kTimeTolerance) {
        imported->Destroy();
        throw std::runtime_error("animation round-trip metadata mismatch");
    }
    imported->Destroy();
    return result;
}

std::string json_escape(const std::string& value) {
    std::ostringstream out;
    for (const unsigned char character : value) {
        switch (character) {
            case '\\': out << "\\\\"; break;
            case '"': out << "\\\""; break;
            case '\b': out << "\\b"; break;
            case '\f': out << "\\f"; break;
            case '\n': out << "\\n"; break;
            case '\r': out << "\\r"; break;
            case '\t': out << "\\t"; break;
            default:
                if (character < 0x20) {
                    out << "\\u" << std::hex << std::setw(4) << std::setfill('0')
                        << static_cast<int>(character) << std::dec;
                } else {
                    out << character;
                }
        }
    }
    return out.str();
}

void write_model_report(
    const std::filesystem::path& report_path,
    const std::filesystem::path& input_path,
    const std::filesystem::path& output_path,
    const Character& character,
    const ModelBuildResult& expected,
    const ModelRoundTripResult& actual,
    int writer_format,
    const char* writer_description) {
    std::ofstream out(report_path, std::ios::binary);
    if (!out) {
        throw std::runtime_error("cannot create report: " + report_path.string());
    }
    out << "{\n"
        << "  \"schema\": \"eurochef-fbx-character-report-v2\",\n"
        << "  \"asset_type\": \"skeletal_mesh\",\n"
        << "  \"source_ir\": \"" << json_escape(input_path.string()) << "\",\n"
        << "  \"output_file\": \"" << json_escape(output_path.string()) << "\",\n"
        << "  \"source_edb_uid\": \"0x" << std::uppercase << std::hex
        << std::setw(8) << std::setfill('0') << character.source_edb_uid << "\",\n"
        << "  \"animskin_uid\": \"0x" << std::setw(8) << character.animskin_uid
        << "\",\n" << std::dec
        << "  \"fbx_sdk_version\": \"" << FBXSDK_VERSION_MAJOR << "."
        << FBXSDK_VERSION_MINOR << "." << FBXSDK_VERSION_POINT << "\",\n"
        << "  \"writer_format\": " << writer_format << ",\n"
        << "  \"writer_description\": \""
        << json_escape(writer_description ? writer_description : "unknown") << "\",\n"
        << "  \"encoding\": \"binary\",\n"
        << "  \"axis_system\": \"MayaZUp\",\n"
        << "  \"units\": \"centimeters\",\n"
        << "  \"contains_animation\": false,\n"
        << "  \"expected\": {\n"
        << "    \"meshes\": " << character.meshes.size() << ",\n"
        << "    \"bones\": " << character.bones.size() << ",\n"
        << "    \"vertices\": " << expected.vertex_count << ",\n"
        << "    \"triangles\": " << expected.triangle_count << ",\n"
        << "    \"clusters\": " << expected.cluster_count << "\n"
        << "  },\n"
        << "  \"round_trip\": {\n"
        << "    \"bones\": " << actual.bone_count << ",\n"
        << "    \"vertices\": " << actual.vertex_count << ",\n"
        << "    \"triangles\": " << actual.triangle_count << ",\n"
        << "    \"clusters\": " << actual.cluster_count << ",\n"
        << "    \"bind_poses\": " << actual.bind_pose_count << ",\n"
        << "    \"status\": \"pass\"\n"
        << "  }\n"
        << "}\n";
}

void write_animation_report(
    const std::filesystem::path& report_path,
    const std::filesystem::path& input_path,
    const std::filesystem::path& output_path,
    const Character& character,
    const AnimationClip& clip,
    const AnimationBuildResult& expected,
    const AnimationRoundTripResult& actual,
    int writer_format,
    const char* writer_description) {
    std::ofstream out(report_path, std::ios::binary);
    if (!out) {
        throw std::runtime_error("cannot create report: " + report_path.string());
    }
    out << "{\n"
        << "  \"schema\": \"eurochef-fbx-animation-report-v1\",\n"
        << "  \"asset_type\": \"animation_only\",\n"
        << "  \"source_ir\": \"" << json_escape(input_path.string()) << "\",\n"
        << "  \"output_file\": \"" << json_escape(output_path.string()) << "\",\n"
        << "  \"source_edb_uid\": \"0x" << std::uppercase << std::hex
        << std::setw(8) << std::setfill('0') << character.source_edb_uid << "\",\n"
        << "  \"animskin_uid\": \"0x" << std::setw(8) << character.animskin_uid << "\",\n"
        << "  \"animation_uid\": \"0x" << std::setw(8) << clip.animation_uid << "\",\n"
        << "  \"source_script_uid\": \"0x" << std::setw(8) << clip.source_script_uid
        << "\",\n" << std::dec
        << "  \"clip_name\": \"" << json_escape(clip.name) << "\",\n"
        << "  \"source_animation_index\": " << clip.source_animation_index << ",\n"
        << "  \"source_script_command\": " << clip.source_script_command << ",\n"
        << "  \"usage_count\": " << clip.usage_count << ",\n"
        << "  \"source_script_fps\": " << clip.source_script_fps << ",\n"
        << "  \"source_command_length\": " << clip.source_command_length << ",\n"
        << "  \"sample_rate\": " << clip.sample_rate << ",\n"
        << "  \"frame_count\": " << clip.frame_count << ",\n"
        << "  \"duration_seconds\": " << clip.duration_seconds << ",\n"
        << "  \"root_motion_mode\": \"" << json_escape(clip.root_motion_mode) << "\",\n"
        << "  \"fbx_sdk_version\": \"" << FBXSDK_VERSION_MAJOR << "."
        << FBXSDK_VERSION_MINOR << "." << FBXSDK_VERSION_POINT << "\",\n"
        << "  \"writer_format\": " << writer_format << ",\n"
        << "  \"writer_description\": \""
        << json_escape(writer_description ? writer_description : "unknown") << "\",\n"
        << "  \"encoding\": \"binary\",\n"
        << "  \"axis_system\": \"MayaZUp\",\n"
        << "  \"units\": \"centimeters\",\n"
        << "  \"expected\": {\n"
        << "    \"bones\": " << character.bones.size() << ",\n"
        << "    \"curves\": " << expected.curve_count << ",\n"
        << "    \"keys\": " << expected.key_count << "\n"
        << "  },\n"
        << "  \"round_trip\": {\n"
        << "    \"bones\": " << actual.bone_count << ",\n"
        << "    \"meshes\": " << actual.mesh_count << ",\n"
        << "    \"anim_stacks\": " << actual.stack_count << ",\n"
        << "    \"anim_layers\": " << actual.layer_count << ",\n"
        << "    \"curves\": " << actual.curve_count << ",\n"
        << "    \"keys\": " << actual.key_count << ",\n"
        << "    \"bind_poses\": " << actual.bind_pose_count << ",\n"
        << "    \"custom_time_mode\": " << (actual.custom_time_mode ? "true" : "false") << ",\n"
        << "    \"custom_frame_rate\": " << actual.custom_frame_rate << ",\n"
        << "    \"start_seconds\": " << actual.start_seconds << ",\n"
        << "    \"stop_seconds\": " << actual.stop_seconds << ",\n"
        << "    \"hierarchy_reference_pose_validation\": \"pass\",\n"
        << "    \"first_last_key_validation\": \"pass\",\n"
        << "    \"status\": \"pass\"\n"
        << "  }\n"
        << "}\n";
}

void validate_model_counts(
    const Character& character,
    const ModelBuildResult& expected,
    const ModelRoundTripResult& actual) {
    if (actual.bone_count != character.bones.size()
        || actual.vertex_count != expected.vertex_count
        || actual.triangle_count != expected.triangle_count
        || actual.cluster_count != expected.cluster_count
        || actual.bind_pose_count == 0) {
        std::ostringstream message;
        message << "FBX model round-trip mismatch: bones " << actual.bone_count << "/"
                << character.bones.size() << ", vertices " << actual.vertex_count << "/"
                << expected.vertex_count << ", triangles " << actual.triangle_count << "/"
                << expected.triangle_count << ", clusters " << actual.cluster_count << "/"
                << expected.cluster_count << ", bind poses " << actual.bind_pose_count;
        throw std::runtime_error(message.str());
    }
}

void export_scene(
    FbxManager* manager,
    FbxScene* scene,
    const std::filesystem::path& output_path,
    int writer_format) {
    FbxExporter* exporter = FbxExporter::Create(manager, "EuroChefFbxExporter");
    if (!exporter->Initialize(
            output_path.string().c_str(), writer_format, manager->GetIOSettings())) {
        const std::string error = exporter->GetStatus().GetErrorString();
        exporter->Destroy();
        throw std::runtime_error("FbxExporter::Initialize failed: " + error);
    }
    if (!exporter->Export(scene)) {
        const std::string error = exporter->GetStatus().GetErrorString();
        exporter->Destroy();
        throw std::runtime_error("FbxExporter::Export failed: " + error);
    }
    exporter->Destroy();
}

int export_model(
    const std::filesystem::path& input_path,
    const std::filesystem::path& output_path,
    const std::filesystem::path& report_path) {
    const Character character = read_character(input_path);
    FbxManager* manager = FbxManager::Create();
    if (!manager) {
        throw std::runtime_error("FbxManager::Create failed");
    }
    configure_export_io(manager, false);
    FbxScene* scene = FbxScene::Create(manager, character.name.c_str());
    const ModelBuildResult expected = build_model_scene(scene, character);
    const int writer_format = binary_writer_format(manager);
    const char* writer_description =
        manager->GetIOPluginRegistry()->GetWriterFormatDescription(writer_format);
    export_scene(manager, scene, output_path, writer_format);
    scene->Destroy();
    const ModelRoundTripResult actual = validate_model_round_trip(manager, output_path);
    validate_model_counts(character, expected, actual);
    write_model_report(
        report_path,
        input_path,
        output_path,
        character,
        expected,
        actual,
        writer_format,
        writer_description);
    manager->Destroy();
    std::cout << "Exported model " << output_path.string() << " (" << expected.triangle_count
              << " triangles, " << expected.vertex_count << " vertices, "
              << character.bones.size() << " bones)\n";
    return 0;
}

int export_animation(
    std::size_t clip_index,
    const std::filesystem::path& input_path,
    const std::filesystem::path& output_path,
    const std::filesystem::path& report_path) {
    const Character character = read_character(input_path);
    if (clip_index >= character.clips.size()) {
        throw std::runtime_error("animation clip index is outside IR clip array");
    }
    const AnimationClip& clip = character.clips[clip_index];
    FbxManager* manager = FbxManager::Create();
    if (!manager) {
        throw std::runtime_error("FbxManager::Create failed");
    }
    configure_export_io(manager, true);
    FbxScene* scene = FbxScene::Create(manager, clip.name.c_str());
    const AnimationBuildResult expected = build_animation_scene(scene, character, clip);
    const int writer_format = binary_writer_format(manager);
    const char* writer_description =
        manager->GetIOPluginRegistry()->GetWriterFormatDescription(writer_format);
    export_scene(manager, scene, output_path, writer_format);
    scene->Destroy();
    const AnimationRoundTripResult actual =
        validate_animation_round_trip(manager, output_path, character, clip);
    if (actual.curve_count != expected.curve_count || actual.key_count != expected.key_count) {
        manager->Destroy();
        throw std::runtime_error("animation round-trip curve/key count mismatch");
    }
    write_animation_report(
        report_path,
        input_path,
        output_path,
        character,
        clip,
        expected,
        actual,
        writer_format,
        writer_description);
    manager->Destroy();
    std::cout << "Exported animation " << output_path.string() << " (" << clip.frame_count
              << " frames, " << clip.sample_rate << " fps, " << clip.duration_seconds
              << " seconds, " << character.bones.size() << " bones)\n";
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    try {
        if (argc == 5 && std::string(argv[1]) == "model") {
            return export_model(argv[2], argv[3], argv[4]);
        }
        if (argc == 6 && std::string(argv[1]) == "animation") {
            const std::size_t clip_index = std::stoull(argv[2]);
            return export_animation(clip_index, argv[3], argv[4], argv[5]);
        }
        std::cerr
            << "Usage:\n"
            << "  fbx_export_helper model <input.ecfbx> <output.fbx> <report.json>\n"
            << "  fbx_export_helper animation <clip_index> <input.ecfbx> <output.fbx> <report.json>\n";
        return 2;
    } catch (const std::exception& error) {
        std::cerr << "FBX export failed: " << error.what() << '\n';
        return 1;
    }
}
