import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
public class CharacterPhysicsMoveDeepEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address a=toAddr(0x0041EC80L); Function f=getFunctionContaining(a);
  println("FUNCTION="+(f==null?"<missing>":f.getPrototypeString(true,false)+" CC="+f.getCallingConventionName()));
  if(f!=null){DecompInterface d=new DecompInterface();d.openProgram(currentProgram);DecompileResults r=d.decompileFunction(f,120,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());d.dispose();}
  InstructionIterator it=currentProgram.getListing().getInstructions(a,true);int n=0;while(it.hasNext()&&n<110){Instruction ins=it.next();println(ins.getAddress()+"  "+ins);n++;}
 }
}