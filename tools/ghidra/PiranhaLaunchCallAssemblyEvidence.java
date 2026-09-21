import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
public class PiranhaLaunchCallAssemblyEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address s=toAddr(0x00467880L); disassemble(s);
  InstructionIterator it=currentProgram.getListing().getInstructions(s,true);
  int n=0;
  while(it.hasNext()&&n<95){
   Instruction ins=it.next(); println(ins.getAddress()+"  "+ins);
   for(Reference r:ins.getReferencesFrom()) println("  REF "+r.getReferenceType()+" -> "+r.getToAddress());
   n++;
  }
 }
}